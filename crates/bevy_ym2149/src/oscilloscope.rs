use bevy::prelude::*;

/// Rolling buffer of oscilloscope samples emitted by the YM2149 playback systems.
#[derive(Resource, Clone)]
pub struct OscilloscopeBuffer {
    samples: Vec<[f32; 3]>,
    capacity: usize,
    index: usize,
}

impl OscilloscopeBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            samples: vec![[0.0; 3]; capacity],
            capacity,
            index: 0,
        }
    }

    pub fn push_sample(&mut self, sample: [f32; 3]) {
        self.samples[self.index] = [
            sample[0].clamp(-1.0, 1.0),
            sample[1].clamp(-1.0, 1.0),
            sample[2].clamp(-1.0, 1.0),
        ];
        self.index = (self.index + 1) % self.capacity;
    }

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
