//! Streaming audio playback with minimal memory consumption
//!
//! This module provides real-time audio playback with a ring buffer that allows
//! concurrent sample generation and playback. Memory usage is limited to the ring buffer size.

// Allow unused methods - these are part of a complete streaming API
#![allow(dead_code)]

pub mod audio_device;
pub mod realtime;
pub mod ring_buffer;

pub use audio_device::AudioDevice;
pub use realtime::{PlaybackStats, RealtimePlayer};
pub use ring_buffer::RingBuffer;

/// Default sample rate (44.1 kHz)
pub const DEFAULT_SAMPLE_RATE: u32 = 44100;

/// Visualization update interval in milliseconds
pub const VISUALIZATION_UPDATE_MS: u64 = 50;

/// Buffer backoff time in microseconds
pub const BUFFER_BACKOFF_MICROS: u64 = 100;

/// Configuration for streaming playback
#[derive(Debug, Clone, Copy)]
pub struct StreamConfig {
    /// Size of the ring buffer (in samples)
    /// Larger buffers = more latency but less chance of underrun
    /// Typical: 4096-16384 samples (93ms-372ms at 44.1kHz)
    pub ring_buffer_size: usize,

    /// Sample rate in Hz
    pub sample_rate: u32,

    /// Number of audio channels
    pub channels: u16,
}

impl StreamConfig {
    /// Create a streaming configuration optimized for low latency
    /// Buffer = 4096 samples ≈ 93ms @ 44.1kHz
    pub fn low_latency(sample_rate: u32) -> Self {
        StreamConfig {
            ring_buffer_size: 4096,
            sample_rate,
            channels: 1,
        }
    }

    /// Create a streaming configuration optimized for stability
    /// Buffer = 16384 samples ≈ 372ms @ 44.1kHz
    pub fn stable(sample_rate: u32) -> Self {
        StreamConfig {
            ring_buffer_size: 16384,
            sample_rate,
            channels: 1,
        }
    }

    /// Get latency in milliseconds
    pub fn latency_ms(&self) -> f32 {
        ((self.ring_buffer_size as f32) / (self.sample_rate as f32)) * 1000.0
    }
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self::stable(44100)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_config_latency() {
        let config = StreamConfig::low_latency(44100);
        let latency = config.latency_ms();
        assert!(latency > 90.0 && latency < 95.0);
    }
}
