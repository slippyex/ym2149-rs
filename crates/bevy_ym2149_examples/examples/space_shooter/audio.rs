//! Audio integration for GIST sound effects

use bevy::audio::{Decodable, Source};
use bevy::prelude::*;
use std::sync::{Arc, Mutex};
use ym2149_gist_replayer::GistPlayer;

#[derive(Asset, TypePath, Clone)]
pub struct GistAudio {
    pub player: Arc<Mutex<GistPlayer>>,
    pub volume: f32,
}

pub struct GistDec {
    player: Arc<Mutex<GistPlayer>>,
    volume: f32,
}

impl Decodable for GistAudio {
    type DecoderItem = f32;
    type Decoder = GistDec;

    fn decoder(&self) -> Self::Decoder {
        GistDec {
            player: Arc::clone(&self.player),
            volume: self.volume,
        }
    }
}

impl Iterator for GistDec {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        Some(self.player.lock().unwrap().generate_samples(1)[0] * self.volume)
    }
}

impl Source for GistDec {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        1
    }

    fn sample_rate(&self) -> u32 {
        44100
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }

    fn try_seek(&mut self, _: std::time::Duration) -> Result<(), bevy::audio::SeekError> {
        Ok(())
    }
}
