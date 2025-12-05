//! Real-time audio streaming and playback control.
//!
//! This module manages:
//! - Audio device initialization
//! - Producer thread for sample generation
//! - Real-time buffer management
//! - Playback state synchronization

use crate::RealtimeChip;
use crate::audio::{AudioDevice, BUFFER_BACKOFF_MICROS, RealtimePlayer, StreamConfig};
use crate::tui::CaptureBuffer;
use parking_lot::Mutex;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

#[derive(Clone, Copy)]
struct ColorFilter {
    enabled: bool,
    z1: f32,
    z2: f32,
}

impl ColorFilter {
    fn new(enabled: bool) -> Self {
        Self {
            enabled,
            z1: 0.0,
            z2: 0.0,
        }
    }

    fn process(&mut self, samples: &mut [f32]) {
        if !self.enabled {
            return;
        }
        for sample in samples.iter_mut() {
            let filtered = (self.z2 * 0.25) + (self.z1 * 0.5) + (*sample * 0.25);
            self.z2 = self.z1;
            self.z1 = *sample;
            *sample = filtered;
        }
    }
}

/// Audio streaming context with device and producer thread.
pub struct StreamingContext {
    /// Audio device handle
    pub audio_device: AudioDevice,
    /// Producer thread handle
    pub producer_thread: std::thread::JoinHandle<()>,
    /// Flag to signal shutdown
    pub running: Arc<AtomicBool>,
    /// Shared player instance
    pub player: Arc<Mutex<Box<dyn RealtimeChip>>>,
    /// Streaming engine
    pub streamer: Arc<RealtimePlayer>,
    /// Capture buffer for TUI visualization (optional)
    pub capture: Option<Arc<Mutex<CaptureBuffer>>>,
    /// Master volume (0-100 as percentage, stored as atomic for thread safety)
    pub volume: Arc<AtomicU32>,
}

impl StreamingContext {
    /// Initialize audio streaming and start producer thread.
    ///
    /// # Arguments
    /// * `player` - The player instance to stream from
    /// * `config` - Streaming configuration
    /// * `color_filter_enabled` - Whether to apply ST color filter
    ///
    /// # Returns
    /// Streaming context with running audio device and producer thread
    pub fn start(
        player: Box<dyn RealtimeChip>,
        config: StreamConfig,
        color_filter_enabled: bool,
    ) -> ym2149_ym_replayer::Result<Self> {
        Self::start_internal(player, config, color_filter_enabled, None, true)
    }

    /// Initialize audio streaming with optional capture buffer for TUI.
    ///
    /// # Arguments
    /// * `player` - The player instance to stream from
    /// * `config` - Streaming configuration
    /// * `color_filter_enabled` - Whether to apply ST color filter
    /// * `capture` - Optional capture buffer for waveform/spectrum visualization
    ///
    /// # Returns
    /// Streaming context with running audio device and producer thread
    pub fn start_with_capture(
        player: Box<dyn RealtimeChip>,
        config: StreamConfig,
        color_filter_enabled: bool,
        capture: Option<Arc<Mutex<CaptureBuffer>>>,
    ) -> ym2149_ym_replayer::Result<Self> {
        Self::start_internal(player, config, color_filter_enabled, capture, true)
    }

    /// Initialize audio streaming paused (for playlist mode).
    ///
    /// The player will not start automatically - call `player.play()` to begin playback.
    pub fn start_paused(
        player: Box<dyn RealtimeChip>,
        config: StreamConfig,
        color_filter_enabled: bool,
        capture: Option<Arc<Mutex<CaptureBuffer>>>,
    ) -> ym2149_ym_replayer::Result<Self> {
        Self::start_internal(player, config, color_filter_enabled, capture, false)
    }

    fn start_internal(
        player: Box<dyn RealtimeChip>,
        config: StreamConfig,
        color_filter_enabled: bool,
        capture: Option<Arc<Mutex<CaptureBuffer>>>,
        auto_start: bool,
    ) -> ym2149_ym_replayer::Result<Self> {
        let streamer = Arc::new(
            RealtimePlayer::new(config)
                .map_err(|e| format!("Failed to create realtime player: {e}"))?,
        );
        let audio_device =
            AudioDevice::new(config.sample_rate, config.channels, streamer.get_buffer())
                .map_err(|e| format!("Failed to create audio device: {e}"))?;

        let player = Arc::new(Mutex::new(player));
        let running = Arc::new(AtomicBool::new(true));
        let volume = Arc::new(AtomicU32::new(100)); // 100% default

        let running_clone = Arc::clone(&running);
        let player_clone = Arc::clone(&player);
        let streamer_clone = Arc::clone(&streamer);
        let volume_clone = Arc::clone(&volume);

        let producer_thread = std::thread::spawn(move || {
            run_producer_loop(
                player_clone,
                streamer_clone,
                running_clone,
                ColorFilter::new(color_filter_enabled),
                auto_start,
                volume_clone,
            );
        });

        Ok(StreamingContext {
            audio_device,
            producer_thread,
            running,
            player,
            streamer,
            capture,
            volume,
        })
    }

    /// Set the master volume (0.0 to 1.0)
    pub fn set_volume(&self, vol: f32) {
        let percentage = (vol.clamp(0.0, 1.0) * 100.0) as u32;
        self.volume.store(percentage, Ordering::Relaxed);
    }

    /// Get the current master volume (0.0 to 1.0)
    #[allow(dead_code)]
    pub fn get_volume(&self) -> f32 {
        self.volume.load(Ordering::Relaxed) as f32 / 100.0
    }

    /// Replace the current player with a new one.
    ///
    /// This allows switching songs without restarting the audio stream.
    /// The new player will start playing immediately.
    pub fn replace_player(&self, new_player: Box<dyn RealtimeChip>) {
        let mut guard = self.player.lock();
        // Stop old player
        guard.stop();
        // Replace with new player
        *guard = new_player;
        // Start new player
        guard.play();
    }

    /// Signal shutdown and wait for producer thread to finish.
    ///
    /// This method handles thread panics gracefully to ensure terminal cleanup
    /// always occurs, even if the producer thread crashed.
    pub fn shutdown(self) {
        self.running.store(false, Ordering::Relaxed);
        if let Err(e) = self.producer_thread.join() {
            // Log but don't panic - we need to clean up the audio device
            eprintln!("Warning: Producer thread panicked during shutdown: {:?}", e);
        }
        self.audio_device.finish();
    }
}

/// Producer loop that generates samples and feeds them to the streamer.
///
/// Runs in a dedicated thread, continuously generating audio samples
/// from the player and writing them to the ring buffer.
fn run_producer_loop(
    player: Arc<Mutex<Box<dyn RealtimeChip>>>,
    streamer: Arc<RealtimePlayer>,
    running: Arc<AtomicBool>,
    mut color_filter: ColorFilter,
    auto_start: bool,
    volume: Arc<AtomicU32>,
) {
    let mut sample_buffer = [0.0f32; 4096];

    // Start playback (unless in paused mode for playlist)
    if auto_start {
        let mut player = player.lock();
        player.play();
    }

    while running.load(Ordering::Relaxed) {
        let batch_size = sample_buffer.len();

        // Generate samples (zero-allocation: reuse sample_buffer)
        {
            let mut player = player.lock();

            // Check for unsupported format
            if let Some(reason) = player.unsupported_reason() {
                eprintln!("{reason}");
                running.store(false, Ordering::Relaxed);
                break;
            }

            // Generate samples (produces silence when stopped/paused)
            player.generate_samples_into(&mut sample_buffer);
        }

        color_filter.process(&mut sample_buffer[..batch_size]);

        // Apply master volume
        let vol = volume.load(Ordering::Relaxed) as f32 / 100.0;
        if vol < 1.0 {
            for sample in sample_buffer[..batch_size].iter_mut() {
                *sample *= vol;
            }
        }

        // Write to ring buffer
        let written = streamer.write_blocking(&sample_buffer[..batch_size]);
        if written < batch_size {
            // Buffer full, back off briefly
            std::thread::sleep(std::time::Duration::from_micros(BUFFER_BACKOFF_MICROS));
        }
    }
}
