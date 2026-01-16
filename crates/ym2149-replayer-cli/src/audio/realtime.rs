//! Real-time audio playback with streaming
//!
//! Provides a simple streaming interface for real-time sample playback.

use super::ring_buffer::RingBufferError;
use super::{BUFFER_BACKOFF_MICROS, RingBuffer, StreamConfig};
use parking_lot::Mutex;
use std::sync::Arc;

/// Real-time audio player with streaming
pub struct RealtimePlayer {
    /// Ring buffer for sample storage
    buffer: Arc<RingBuffer>,
    /// Playback statistics
    stats: Arc<Mutex<PlaybackStats>>,
}

/// Playback statistics for monitoring overruns and buffer health
#[derive(Debug, Clone, Copy)]
pub struct PlaybackStats {
    /// Number of overrun events (producer write failed due to full buffer)
    pub overrun_count: usize,
    /// Number of samples played
    pub samples_played: usize,
    /// Current buffer fill percentage
    pub fill_percentage: f32,
}

impl RealtimePlayer {
    /// Create a new real-time player with streaming
    pub fn new(config: StreamConfig) -> Result<Self, RingBufferError> {
        let buffer = Arc::new(RingBuffer::new(config.ring_buffer_size)?);

        let stats = Arc::new(Mutex::new(PlaybackStats {
            overrun_count: 0,
            samples_played: 0,
            fill_percentage: 0.0,
        }));

        Ok(RealtimePlayer { buffer, stats })
    }

    /// Write samples to the playback buffer
    /// Blocks with backpressure until all samples are written or max retries exceeded.
    /// Returns number of samples actually written.
    pub fn write_blocking(&self, samples: &[f32]) -> usize {
        const MAX_RETRIES: u32 = 1000; // ~100ms max wait at 100µs backoff

        let mut total_written = 0;
        let mut remaining = samples;
        let mut retry_count = 0;

        // Keep retrying until all samples are written or max retries exceeded
        while !remaining.is_empty() && retry_count < MAX_RETRIES {
            let written = self.buffer.write(remaining);

            {
                // Update stats
                let mut stats = self.stats.lock();
                stats.samples_played += written;
                stats.fill_percentage = self.buffer.fill_percentage();
            }

            total_written += written;

            if written == 0 {
                // Buffer is full, back off and retry
                std::thread::sleep(std::time::Duration::from_micros(BUFFER_BACKOFF_MICROS));
                retry_count += 1;
            } else {
                // Successfully wrote some samples, reset retry count
                remaining = &remaining[written..];
                retry_count = 0;
            }
        }

        total_written
    }

    /// Get current playback statistics
    pub fn get_stats(&self) -> PlaybackStats {
        *self.stats.lock()
    }

    /// Get buffer fill percentage (0.0 to 1.0)
    pub fn fill_percentage(&self) -> f32 {
        self.buffer.fill_percentage()
    }

    /// Get reference to the ring buffer for audio device integration
    /// This allows the audio device to read samples as they're produced
    pub fn get_buffer(&self) -> Arc<RingBuffer> {
        Arc::clone(&self.buffer)
    }
}

impl Drop for RealtimePlayer {
    fn drop(&mut self) {
        // Stream is automatically stopped when dropped
        let stats = self.stats.lock();
        println!(
            "Playback complete: {} samples, {} overruns",
            stats.samples_played, stats.overrun_count
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_config() {
        let config = StreamConfig::stable(44100);
        assert!(config.latency_ms() > 300.0);
    }

    #[test]
    fn test_playback_stats() {
        let stats = PlaybackStats {
            overrun_count: 0,
            samples_played: 44100,
            fill_percentage: 0.5,
        };

        assert_eq!(stats.samples_played, 44100);
        assert_eq!(stats.overrun_count, 0);
        assert!(stats.fill_percentage > 0.4 && stats.fill_percentage < 0.6);
    }
}
