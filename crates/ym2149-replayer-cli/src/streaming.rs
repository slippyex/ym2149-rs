//! Real-time audio streaming and playback control.
//!
//! This module manages:
//! - Audio device initialization
//! - Producer thread for sample generation
//! - Real-time buffer management
//! - Playback state synchronization

use crate::audio::{AudioDevice, BUFFER_BACKOFF_MICROS, RealtimePlayer, StreamConfig};
use parking_lot::Mutex;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use ym2149_ym_replayer::PlaybackState;

use crate::RealtimeChip;

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
}

impl StreamingContext {
    /// Initialize audio streaming and start producer thread.
    ///
    /// # Arguments
    /// * `player` - The player instance to stream from
    /// * `config` - Streaming configuration
    ///
    /// # Returns
    /// Streaming context with running audio device and producer thread
    pub fn start(
        player: Box<dyn RealtimeChip>,
        config: StreamConfig,
        color_filter_enabled: bool,
    ) -> ym2149_ym_replayer::Result<Self> {
        let streamer = Arc::new(
            RealtimePlayer::new(config)
                .map_err(|e| format!("Failed to create realtime player: {e}"))?,
        );
        let audio_device =
            AudioDevice::new(config.sample_rate, config.channels, streamer.get_buffer())
                .map_err(|e| format!("Failed to create audio device: {e}"))?;

        println!("Audio device initialized - playing to speakers\n");

        let player = Arc::new(Mutex::new(player));
        let running = Arc::new(AtomicBool::new(true));

        let running_clone = Arc::clone(&running);
        let player_clone = Arc::clone(&player);
        let streamer_clone = Arc::clone(&streamer);

        let producer_thread = std::thread::spawn(move || {
            run_producer_loop(
                player_clone,
                streamer_clone,
                running_clone,
                ColorFilter::new(color_filter_enabled),
            );
        });

        Ok(StreamingContext {
            audio_device,
            producer_thread,
            running,
            player,
            streamer,
        })
    }

    /// Signal shutdown and wait for producer thread to finish.
    pub fn shutdown(self) {
        self.running.store(false, Ordering::Relaxed);
        self.producer_thread
            .join()
            .expect("Producer thread panicked during shutdown");
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
) {
    let mut sample_buffer = [0.0f32; 4096];

    // Start playback
    {
        let mut player = player.lock();
        player.play();
    }

    while running.load(Ordering::Relaxed) {
        let batch_size = sample_buffer.len();

        // Generate samples (zero-allocation: reuse sample_buffer)
        {
            let mut player = player.lock();

            // Restart if stopped
            if player.state() == PlaybackState::Stopped {
                if let Some(reason) = player.unsupported_reason() {
                    eprintln!("{reason}");
                    running.store(false, Ordering::Relaxed);
                    break;
                }
                player.stop();
                player.play();
            }

            // Use zero-allocation API - no Vec allocation, no copy
            player.generate_samples_into(&mut sample_buffer);

            if let Some(reason) = player.unsupported_reason() {
                eprintln!("{reason}");
                running.store(false, Ordering::Relaxed);
                break;
            }
        }

        color_filter.process(&mut sample_buffer[..batch_size]);

        // Write to ring buffer
        let written = streamer.write_blocking(&sample_buffer[..batch_size]);
        if written < batch_size {
            // Buffer full, back off briefly
            std::thread::sleep(std::time::Duration::from_micros(BUFFER_BACKOFF_MICROS));
        }
    }
}
