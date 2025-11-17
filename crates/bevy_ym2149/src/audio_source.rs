//! YM2149 audio source asset type for Bevy

use bevy::asset::{Asset, AssetLoader, LoadContext};
use bevy::audio::{Decodable, Source};
use bevy::reflect::TypePath;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

use crate::error::{BevyYm2149Error, Result};
use crate::song_player::{SharedSongPlayer, load_song_from_bytes};

/// A loaded YM2149 audio file ready to be played
///
/// This asset implements both Asset (for loading) and Decodable (for playback).
/// The player is shared via Arc<RwLock<>> to allow both the audio thread
/// and Bevy systems to access it with reduced lock contention.
#[derive(Asset, TypePath, Clone)]
pub struct Ym2149AudioSource {
    /// The raw YM file data
    pub data: Vec<u8>,
    /// Cached metadata about the YM file
    pub metadata: Ym2149Metadata,
    /// Shared YM player instance for audio generation
    player: SharedSongPlayer,
    /// Shared stereo gains (left, right) applied during decoding
    stereo_gain: Arc<RwLock<(f32, f32)>>,
    /// Sample rate for playback
    sample_rate: u32,
    /// Total number of samples in the track
    total_samples: usize,
}

/// Metadata about a YM2149 audio file
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Ym2149Metadata {
    /// Song title
    pub title: String,
    /// Song author
    pub author: String,
    /// Comments/notes
    pub comment: String,
    /// Number of frames in the song
    pub frame_count: usize,
    /// Duration in seconds (approximate)
    pub duration_seconds: f32,
}

impl Ym2149AudioSource {
    /// Create a new audio source from raw YM file data
    pub fn new(data: Vec<u8>) -> Result<Self> {
        Self::new_with_gains(data, Arc::new(RwLock::new((1.0, 1.0))))
    }

    /// Create a new audio source from raw YM file data with shared stereo gains
    pub fn new_with_gains(data: Vec<u8>, stereo_gain: Arc<RwLock<(f32, f32)>>) -> Result<Self> {
        // Load the song to create a player
        let (mut player, metrics, metadata) =
            load_song_from_bytes(&data).map_err(BevyYm2149Error::MetadataExtraction)?;

        player
            .play()
            .map_err(|e| BevyYm2149Error::Other(format!("Failed to start player: {}", e)))?;

        let sample_rate = crate::playback::YM2149_SAMPLE_RATE;
        let total_samples = metrics.total_samples();

        Ok(Self {
            data,
            metadata,
            player: Arc::new(RwLock::new(player)),
            stereo_gain,
            sample_rate,
            total_samples,
        })
    }

    pub(crate) fn from_shared_player(
        player: SharedSongPlayer,
        metadata: Ym2149Metadata,
        total_samples: usize,
        stereo_gain: Arc<RwLock<(f32, f32)>>,
    ) -> Self {
        Self {
            data: Vec::new(),
            metadata,
            player,
            stereo_gain,
            sample_rate: crate::playback::YM2149_SAMPLE_RATE,
            total_samples,
        }
    }

    /// Get the duration of this audio source in seconds
    pub fn duration(&self) -> f32 {
        self.metadata.duration_seconds
    }

    /// Get metadata for this audio source
    pub fn metadata(&self) -> &Ym2149Metadata {
        &self.metadata
    }

    /// Get a handle to the shared player for external control
    pub fn player(&self) -> SharedSongPlayer {
        Arc::clone(&self.player)
    }
}

/// Error type for YM2149 asset loading
#[derive(Error, Debug)]
#[error("{0}")]
pub struct Ym2149LoadError(String);

/// Asset loader for YM2149 files
#[derive(Default)]
pub struct Ym2149Loader;

impl AssetLoader for Ym2149Loader {
    type Asset = Ym2149AudioSource;
    type Settings = ();
    type Error = Ym2149LoadError;

    async fn load(
        &self,
        reader: &mut dyn bevy::asset::io::Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> std::result::Result<Self::Asset, Self::Error> {
        let mut data = Vec::new();
        reader
            .read_to_end(&mut data)
            .await
            .map_err(|e| Ym2149LoadError(format!("Failed to read asset: {}", e)))?;

        Ym2149AudioSource::new(data).map_err(|e| Ym2149LoadError(e.to_string()))
    }

    fn extensions(&self) -> &[&str] {
        &["ym", "aks"]
    }
}

/// Decoder for YM2149 audio playback
///
/// This decoder implements the `Source` trait from rodio, allowing it to be
/// used directly in Bevy's audio system. It generates samples on-demand by
/// calling the YM player.
pub struct Ym2149Decoder {
    /// Shared reference to the YM player
    player: SharedSongPlayer,
    /// Shared stereo gains (left, right)
    stereo_gain: Arc<RwLock<(f32, f32)>>,
    /// Sample rate in Hz
    sample_rate: u32,
    /// Current sample position
    current_sample: usize,
    /// Total number of samples (mono frames)
    total_samples: usize,
    /// Sample buffer for batch generation
    buffer: Vec<f32>,
    /// Current position in buffer
    buffer_pos: usize,
    /// Scratch buffer for mono samples
    mono_buffer: Vec<f32>,
}

impl Ym2149Decoder {
    /// Create a new decoder
    fn new(
        player: SharedSongPlayer,
        stereo_gain: Arc<RwLock<(f32, f32)>>,
        sample_rate: u32,
        total_samples: usize,
    ) -> Self {
        Self {
            player,
            stereo_gain,
            sample_rate,
            current_sample: 0,
            total_samples,
            buffer: Vec::new(),
            buffer_pos: 0,
            mono_buffer: Vec::new(),
        }
    }

    /// Generate a batch of samples (zero-allocation reuse of internal buffer)
    fn fill_buffer(&mut self) {
        // Generate 882 samples (one VBL frame at 50Hz)
        const SAMPLES_PER_FRAME: usize = 882;

        // Resize buffers if needed (only allocates on first call or size change)
        if self.buffer.len() != SAMPLES_PER_FRAME * 2 {
            self.buffer.resize(SAMPLES_PER_FRAME * 2, 0.0);
        }
        if self.mono_buffer.len() != SAMPLES_PER_FRAME {
            self.mono_buffer.resize(SAMPLES_PER_FRAME, 0.0);
        }

        // Reuse existing buffer - no allocation in hot path
        let mut player = self.player.write();
        player.generate_samples_into(&mut self.mono_buffer);

        let (left_gain, right_gain) = *self.stereo_gain.read();
        for (i, sample) in self.mono_buffer.iter().copied().enumerate() {
            let base = i * 2;
            self.buffer[base] = sample * left_gain;
            self.buffer[base + 1] = sample * right_gain;
        }
        self.buffer_pos = 0;
    }
}

impl Iterator for Ym2149Decoder {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        // Check if we've reached the end
        if self.current_sample >= self.total_samples.saturating_mul(2) {
            return None;
        }

        // Refill buffer if needed
        if self.buffer_pos >= self.buffer.len() {
            self.fill_buffer();
        }

        // Get sample from buffer
        let sample = if self.buffer_pos < self.buffer.len() {
            self.buffer[self.buffer_pos]
        } else {
            0.0 // Silence if buffer is empty
        };

        self.buffer_pos += 1;
        self.current_sample += 1;

        Some(sample)
    }
}

impl Source for Ym2149Decoder {
    fn current_frame_len(&self) -> Option<usize> {
        Some(
            self.total_samples
                .saturating_mul(2)
                .saturating_sub(self.current_sample),
        )
    }

    fn channels(&self) -> u16 {
        2 // Stereo output
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f32(
            self.total_samples as f32 / self.sample_rate as f32,
        ))
    }
}

/// Implement Decodable to integrate with Bevy's audio system
impl Decodable for Ym2149AudioSource {
    type DecoderItem = <Ym2149Decoder as Iterator>::Item;
    type Decoder = Ym2149Decoder;

    fn decoder(&self) -> Self::Decoder {
        Ym2149Decoder::new(
            Arc::clone(&self.player),
            Arc::clone(&self.stereo_gain),
            self.sample_rate,
            self.total_samples,
        )
    }
}
