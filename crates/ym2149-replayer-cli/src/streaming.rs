//! Real-time audio streaming and playback control.
//!
//! This module manages:
//! - Audio device initialization
//! - Producer thread for sample generation
//! - Real-time buffer management
//! - Playback state synchronization
//! - Visualization delay compensation (syncs visuals with audio output)

use crate::audio::{AudioDevice, BUFFER_BACKOFF_MICROS, RealtimePlayer, StreamConfig};
use crate::tui::CaptureBuffer;
use crate::{RealtimeChip, VisualSnapshot};
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// Delay buffer for visual snapshots to sync visualization with audio output.
///
/// The audio ring buffer introduces latency between when samples are generated
/// and when they're actually played. This buffer delays the visual snapshots
/// by the same amount so visualization matches the audible output.
pub struct SnapshotDelayBuffer {
    /// Ring buffer of snapshots
    snapshots: VecDeque<VisualSnapshot>,
    /// Target delay in number of snapshots (based on audio buffer size)
    target_delay: usize,
    /// Current delayed snapshot for TUI to read
    current_delayed: VisualSnapshot,
}

impl SnapshotDelayBuffer {
    /// Create a new delay buffer.
    ///
    /// # Arguments
    /// * `audio_buffer_samples` - Size of the audio ring buffer in samples
    /// * `batch_size` - Number of samples generated per batch (snapshot interval)
    pub fn new(audio_buffer_samples: usize, batch_size: usize) -> Self {
        // Calculate how many batches fit in the audio buffer
        // This is the delay we need to compensate for
        let target_delay = (audio_buffer_samples / batch_size).max(1);

        Self {
            snapshots: VecDeque::with_capacity(target_delay + 2),
            target_delay,
            current_delayed: VisualSnapshot::default(),
        }
    }

    /// Push a new snapshot (called from producer thread after generating samples).
    /// Updates the current_delayed snapshot that the TUI reads.
    pub fn push(&mut self, snapshot: VisualSnapshot) {
        self.snapshots.push_back(snapshot);

        // Update delayed snapshot if we have enough buffered
        if self.snapshots.len() > self.target_delay
            && let Some(delayed) = self.snapshots.pop_front()
        {
            self.current_delayed = delayed;
        }
    }

    /// Get the current delayed snapshot (called from TUI thread).
    pub fn get_delayed(&self) -> VisualSnapshot {
        self.current_delayed
    }

    /// Clear the buffer (e.g., when switching songs).
    pub fn clear(&mut self) {
        self.snapshots.clear();
        self.current_delayed = VisualSnapshot::default();
    }
}

/// ST color filter for stereo audio.
///
/// Simple lowpass filter that smooths the audio output to simulate
/// the analog filtering of the Atari ST's audio hardware.
#[derive(Clone, Copy)]
struct ColorFilter {
    enabled: bool,
    /// Left channel filter state
    z1_l: f32,
    z2_l: f32,
    /// Right channel filter state
    z1_r: f32,
    z2_r: f32,
}

impl ColorFilter {
    fn new(enabled: bool) -> Self {
        Self {
            enabled,
            z1_l: 0.0,
            z2_l: 0.0,
            z1_r: 0.0,
            z2_r: 0.0,
        }
    }

    /// Process interleaved stereo samples in place.
    fn process_stereo(&mut self, samples: &mut [f32]) {
        if !self.enabled {
            return;
        }
        for chunk in samples.chunks_exact_mut(2) {
            // Left channel
            let filtered_l = (self.z2_l * 0.25) + (self.z1_l * 0.5) + (chunk[0] * 0.25);
            self.z2_l = self.z1_l;
            self.z1_l = chunk[0];
            chunk[0] = filtered_l;

            // Right channel
            let filtered_r = (self.z2_r * 0.25) + (self.z1_r * 0.5) + (chunk[1] * 0.25);
            self.z2_r = self.z1_r;
            self.z1_r = chunk[1];
            chunk[1] = filtered_r;
        }
    }
}

/// Batch size for sample generation in frames (stereo frame pairs per visual snapshot).
/// With stereo, this is 2048 frames = 4096 samples (interleaved L/R).
const SAMPLE_BATCH_SIZE: usize = 2048;

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
    /// Delay buffer for syncing visuals with audio output
    pub snapshot_delay: Arc<Mutex<SnapshotDelayBuffer>>,
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

        // Create delay buffer to sync visuals with audio output
        let snapshot_delay = Arc::new(Mutex::new(SnapshotDelayBuffer::new(
            config.ring_buffer_size,
            SAMPLE_BATCH_SIZE,
        )));

        let running_clone = Arc::clone(&running);
        let player_clone = Arc::clone(&player);
        let streamer_clone = Arc::clone(&streamer);
        let volume_clone = Arc::clone(&volume);
        let snapshot_delay_clone = Arc::clone(&snapshot_delay);

        let producer_thread = std::thread::spawn(move || {
            run_producer_loop(
                player_clone,
                streamer_clone,
                running_clone,
                ColorFilter::new(color_filter_enabled),
                auto_start,
                volume_clone,
                snapshot_delay_clone,
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
            snapshot_delay,
        })
    }

    /// Set the master volume (0.0 to 1.0)
    pub fn set_volume(&self, vol: f32) {
        let percentage = (vol.clamp(0.0, 1.0) * 100.0) as u32;
        self.volume.store(percentage, Ordering::Relaxed);
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
        // Clear the snapshot delay buffer for fresh start
        self.snapshot_delay.lock().clear();
    }

    /// Get a delayed visual snapshot that's synced with audio output.
    ///
    /// Call this instead of directly reading from the player to get
    /// visualization that matches what's currently being heard.
    pub fn get_delayed_snapshot(&self) -> VisualSnapshot {
        self.snapshot_delay.lock().get_delayed()
    }

    /// Signal shutdown and wait for producer thread to finish.
    ///
    /// This method handles thread panics gracefully to ensure terminal cleanup
    /// always occurs, even if the producer thread crashed.
    pub fn shutdown(self) {
        self.running.store(false, Ordering::Relaxed);
        if let Err(e) = self.producer_thread.join() {
            // Log but don't panic - we need to clean up the audio device
            eprintln!("Warning: Producer thread panicked during shutdown: {e:?}");
        }
        self.audio_device.finish();
    }
}

/// Producer loop that generates samples and feeds them to the streamer.
///
/// Runs in a dedicated thread, continuously generating stereo audio samples
/// from the player and writing them to the ring buffer. Also captures
/// visual snapshots and pushes them to the delay buffer for sync.
fn run_producer_loop(
    player: Arc<Mutex<Box<dyn RealtimeChip>>>,
    streamer: Arc<RealtimePlayer>,
    running: Arc<AtomicBool>,
    mut color_filter: ColorFilter,
    auto_start: bool,
    volume: Arc<AtomicU32>,
    snapshot_delay: Arc<Mutex<SnapshotDelayBuffer>>,
) {
    // Stereo buffer: 2048 frames * 2 channels = 4096 samples (interleaved L/R)
    let mut sample_buffer = [0.0f32; 4096];

    // Start playback (unless in paused mode for playlist)
    if auto_start {
        let mut player = player.lock();
        player.play();
    }

    while running.load(Ordering::Relaxed) {
        let batch_size = sample_buffer.len();

        // Generate stereo samples and capture snapshot
        let snapshot = {
            let mut player = player.lock();

            // Check for unsupported format
            if let Some(reason) = player.unsupported_reason() {
                eprintln!("{reason}");
                running.store(false, Ordering::Relaxed);
                break;
            }

            // Generate stereo samples (produces silence when stopped/paused)
            player.generate_samples_into_stereo(&mut sample_buffer);

            // Capture visual snapshot AFTER generating samples
            // This is the state that corresponds to the audio we just generated
            player.visual_snapshot()
        };

        // Push snapshot to delay buffer (syncs visualization with audio output)
        snapshot_delay.lock().push(snapshot);

        // Apply color filter to stereo samples
        color_filter.process_stereo(&mut sample_buffer[..batch_size]);

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
