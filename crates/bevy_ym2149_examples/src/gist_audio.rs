//! GIST audio integration helpers for examples.
//!
//! Provides a Bevy `Decodable` audio asset that streams samples from a shared
//! `ym2149_gist_replayer::GistPlayer`.

use bevy::audio::{Decodable, Source};
use bevy::prelude::*;
use std::sync::{Arc, Mutex};
use ym2149_gist_replayer::GistPlayer;

const BLOCK_SAMPLES: usize = 512;

/// Audio source that generates samples from a shared GIST player.
#[derive(Asset, TypePath, Clone)]
pub struct GistAudio {
    /// Shared player (locked briefly when refilling the internal sample buffer).
    pub player: Arc<Mutex<GistPlayer>>,
    /// Linear gain multiplier applied to samples.
    pub volume: f32,
}

pub struct GistDecoder {
    player: Arc<Mutex<GistPlayer>>,
    volume: f32,
    buffer: Vec<f32>,
    cursor: usize,
}

impl Decodable for GistAudio {
    type DecoderItem = f32;
    type Decoder = GistDecoder;

    fn decoder(&self) -> Self::Decoder {
        GistDecoder {
            player: Arc::clone(&self.player),
            volume: self.volume,
            buffer: Vec::new(),
            cursor: 0,
        }
    }
}

impl Iterator for GistDecoder {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor >= self.buffer.len() {
            self.cursor = 0;
            self.buffer.clear();

            let samples = self
                .player
                .lock()
                .ok()
                .map(|mut player| player.generate_samples(BLOCK_SAMPLES))
                .unwrap_or_else(|| vec![0.0; BLOCK_SAMPLES]);
            self.buffer = samples;
        }

        let sample = *self.buffer.get(self.cursor).unwrap_or(&0.0);
        self.cursor += 1;
        Some(sample * self.volume)
    }
}

impl Source for GistDecoder {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        1
    }

    fn sample_rate(&self) -> u32 {
        44_100
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }

    fn try_seek(&mut self, _: std::time::Duration) -> Result<(), bevy::audio::SeekError> {
        Ok(())
    }
}
