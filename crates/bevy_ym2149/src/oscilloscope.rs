//! Oscilloscope buffer for real-time waveform visualization.
//!
//! This module provides a rolling buffer that captures per-channel audio samples
//! for rendering oscilloscope-style visualizations.

use bevy::prelude::*;

/// Rolling buffer of oscilloscope samples emitted by the YM2149 playback systems.
///
/// Stores the most recent samples for each of the three PSG channels (A, B, C).
/// Use [`get_samples`](Self::get_samples) to retrieve samples in chronological order.
#[derive(Resource, Clone)]
pub struct OscilloscopeBuffer {
    samples: Vec<[f32; 3]>,
    capacity: usize,
    index: usize,
}

impl OscilloscopeBuffer {
    /// Creates a new oscilloscope buffer with the specified capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            samples: vec![[0.0; 3]; capacity],
            capacity,
            index: 0,
        }
    }

    /// Pushes a new sample (one value per channel) into the buffer.
    ///
    /// Values are clamped to the range [-1.0, 1.0].
    pub fn push_sample(&mut self, sample: [f32; 3]) {
        self.samples[self.index] = [
            sample[0].clamp(-1.0, 1.0),
            sample[1].clamp(-1.0, 1.0),
            sample[2].clamp(-1.0, 1.0),
        ];
        self.index = (self.index + 1) % self.capacity;
    }

    /// Returns all samples in chronological order (oldest first).
    pub fn get_samples(&self) -> Vec<[f32; 3]> {
        (0..self.capacity)
            .map(|offset| {
                let idx = (self.index + offset) % self.capacity;
                self.samples[idx]
            })
            .collect()
    }
}

impl Default for OscilloscopeBuffer {
    fn default() -> Self {
        Self::new(512)
    }
}
