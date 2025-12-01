//! Real-time audio playback with streaming
//!
//! Provides a simple streaming interface for real-time sample playback.

use super::StreamConfig;
use super::ring_buffer::RingBufferError;
use super::{BUFFER_BACKOFF_MICROS, RingBuffer};
use parking_lot::Mutex;
use std::sync::Arc;

// Re-export PlaybackState from ym2149-common
pub use ym2149_common::PlaybackState;

/// Real-time audio player with streaming
pub struct RealtimePlayer {
    /// Ring buffer for sample storage
    buffer: Arc<RingBuffer>,
    /// Stream configuration
    config: StreamConfig,
    /// Playback statistics
    stats: Arc<Mutex<PlaybackStats>>,
    /// Current playback state
    state: Arc<Mutex<PlaybackState>>,
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

        let state = Arc::new(Mutex::new(PlaybackState::Stopped));

        Ok(RealtimePlayer {
            buffer,
            config,
            stats,
            state,
        })
    }

    /// Write samples to the playback buffer
    /// Blocks indefinitely if buffer is full (backpressure) until all samples are written
    pub fn write_blocking(&self, samples: &[f32]) -> usize {
        let mut total_written = 0;
        let mut remaining = samples;

        // Keep retrying until all samples are written
        while !remaining.is_empty() {
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
            } else {
                // Successfully wrote some samples
                remaining = &remaining[written..];
            }
        }

        total_written
    }

    /// Write samples without blocking (returns 0 if buffer full)
    pub fn write_nonblocking(&self, samples: &[f32]) -> usize {
        let written = self.buffer.write(samples);
        let fill_pct = self.buffer.fill_percentage();

        let mut stats = self.stats.lock();
        if written < samples.len() {
            stats.overrun_count += 1;
        }
        stats.samples_played += written;
        stats.fill_percentage = fill_pct;

        written
    }

    /// Get the number of samples that can be written without blocking
    pub fn available_write(&self) -> usize {
        self.buffer.available_write()
    }

    /// Get current playback statistics
    pub fn get_stats(&self) -> PlaybackStats {
        *self.stats.lock()
    }

    /// Flush the buffer (clear all pending samples)
    pub fn flush(&self) {
        self.buffer.flush();
    }

    /// Get buffer fill percentage (0.0 to 1.0)
    pub fn fill_percentage(&self) -> f32 {
        self.buffer.fill_percentage()
    }

    /// Get buffer latency in milliseconds
    pub fn latency_ms(&self) -> f32 {
        self.config.latency_ms()
    }

    /// Get the stream configuration
    pub fn config(&self) -> &StreamConfig {
        &self.config
    }

    /// Start playback
    pub fn play(&mut self) {
        let mut state = self.state.lock();
        *state = PlaybackState::Playing;
    }

    /// Pause playback
    pub fn pause(&mut self) {
        let mut state = self.state.lock();
        if *state == PlaybackState::Playing {
            *state = PlaybackState::Paused;
        }
    }

    /// Stop playback
    pub fn stop(&mut self) {
        let mut state = self.state.lock();
        *state = PlaybackState::Stopped;
        self.buffer.flush();
    }

    /// Get current playback state
    pub fn state(&self) -> PlaybackState {
        *self.state.lock()
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
