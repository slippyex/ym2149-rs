//! Real-time audio streaming and playback control.
//!
//! This module manages:
//! - Audio device initialization
//! - Producer thread for sample generation
//! - Real-time buffer management
//! - Playback state synchronization

use parking_lot::Mutex;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use ym_replayer::PlaybackState;
use ym2149::streaming::{BUFFER_BACKOFF_MICROS, StreamConfig};
use ym2149::{AudioDevice, RealtimePlayer};

use crate::RealtimeChip;

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
    pub fn start(player: Box<dyn RealtimeChip>, config: StreamConfig) -> ym_replayer::Result<Self> {
        let streamer = Arc::new(RealtimePlayer::new(config)?);
        let audio_device =
            AudioDevice::new(config.sample_rate, config.channels, streamer.get_buffer())?;

        println!("Audio device initialized - playing to speakers\n");

        let player = Arc::new(Mutex::new(player));
        let running = Arc::new(AtomicBool::new(true));

        let running_clone = Arc::clone(&running);
        let player_clone = Arc::clone(&player);
        let streamer_clone = Arc::clone(&streamer);

        let producer_thread = std::thread::spawn(move || {
            run_producer_loop(player_clone, streamer_clone, running_clone);
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
) {
    let mut sample_buffer = [0.0f32; 4096];

    // Start playback
    {
        let mut player = player.lock();
        if let Err(e) = player.play() {
            eprintln!("Failed to start playback: {}", e);
            return;
        }
    }

    while running.load(Ordering::Relaxed) {
        let batch_size = sample_buffer.len();

        // Generate samples (zero-allocation: reuse sample_buffer)
        {
            let mut player = player.lock();

            // Restart if stopped
            if player.state() == PlaybackState::Stopped {
                let _ = player.stop();
                let _ = player.play();
            }

            // Use zero-allocation API - no Vec allocation, no copy
            player.generate_samples_into(&mut sample_buffer);
        }

        // Write to ring buffer
        let written = streamer.write_blocking(&sample_buffer[..batch_size]);
        if written < batch_size {
            // Buffer full, back off briefly
            std::thread::sleep(std::time::Duration::from_micros(BUFFER_BACKOFF_MICROS));
        }
    }
}
