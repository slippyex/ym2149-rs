//! YM2149 audio source asset type for Bevy

use bevy::asset::{Asset, AssetLoader, LoadContext};
use bevy::audio::Decodable;
use bevy::reflect::TypePath;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;

use crate::error::{BevyYm2149Error, Result};
use crate::playback::ToneSettings;
use crate::song_player::{SharedSongPlayer, load_song_from_bytes};
use crate::streaming::{AudioStream, StreamingDecoder};

/// A loaded YM2149 audio file ready to be played
///
/// This asset implements both Asset (for loading) and Decodable (for playback).
/// Audio generation happens in a dedicated producer thread that feeds samples
/// into a ring buffer, eliminating lock contention with the audio thread.
#[derive(Asset, TypePath)]
pub struct Ym2149AudioSource {
    /// The raw YM file data
    pub data: Vec<u8>,
    /// Cached metadata about the YM file
    pub metadata: Ym2149Metadata,
    /// Shared YM player instance (for metadata/control only)
    player: SharedSongPlayer,
    /// Audio streaming infrastructure (producer thread + ring buffer)
    stream: Arc<AudioStream>,
    /// Sample rate for playback
    sample_rate: u32,
    /// Total number of samples in the track
    total_samples: usize,
}

// Manual Clone implementation because AudioStream contains a thread handle
impl Clone for Ym2149AudioSource {
    fn clone(&self) -> Self {
        // Start a new stream for the clone - this creates a new producer thread
        let stream = Arc::new(AudioStream::start(Arc::clone(&self.player)));

        Self {
            data: self.data.clone(),
            metadata: self.metadata.clone(),
            player: Arc::clone(&self.player),
            stream,
            sample_rate: self.sample_rate,
            total_samples: self.total_samples,
        }
    }
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
        // Load the song to create a player
        let (player, metrics, metadata) =
            load_song_from_bytes(&data).map_err(BevyYm2149Error::MetadataExtraction)?;

        let sample_rate = crate::playback::YM2149_SAMPLE_RATE;
        let total_samples = metrics.total_samples();

        let player = Arc::new(parking_lot::RwLock::new(player));

        // Start the audio stream (spawns producer thread)
        let stream = Arc::new(AudioStream::start(Arc::clone(&player)));

        Ok(Self {
            data,
            metadata,
            player,
            stream,
            sample_rate,
            total_samples,
        })
    }

    /// Create a new audio source from raw YM file data (compatibility method for crossfade).
    ///
    /// The stereo_gain and tone_settings parameters are applied to the stream.
    pub fn new_with_shared(
        data: Vec<u8>,
        stereo_gain: Arc<parking_lot::RwLock<(f32, f32)>>,
        tone_settings: Arc<parking_lot::RwLock<ToneSettings>>,
    ) -> Result<Self> {
        Self::new_with_subsong(data, stereo_gain, tone_settings, None)
    }

    /// Create a new audio source from raw YM file data with optional subsong selection.
    ///
    /// The stereo_gain and tone_settings parameters are applied to the stream.
    /// If subsong is Some, the player will be initialized to that subsong (1-based index).
    pub fn new_with_subsong(
        data: Vec<u8>,
        stereo_gain: Arc<parking_lot::RwLock<(f32, f32)>>,
        tone_settings: Arc<parking_lot::RwLock<ToneSettings>>,
        subsong: Option<usize>,
    ) -> Result<Self> {
        // Load the song to create a player
        let (mut player, metrics, metadata) =
            load_song_from_bytes(&data).map_err(BevyYm2149Error::MetadataExtraction)?;

        // Apply subsong selection BEFORE starting the stream
        if let Some(index) = subsong {
            player.set_subsong(index);
        }

        let sample_rate = crate::playback::YM2149_SAMPLE_RATE;
        let total_samples = metrics.total_samples();

        let player = Arc::new(parking_lot::RwLock::new(player));

        // Start the audio stream (spawns producer thread) - now with correct subsong
        let stream = Arc::new(AudioStream::start(Arc::clone(&player)));

        // Apply settings to the stream
        let (left, right) = *stereo_gain.read();
        stream.state.set_stereo_gain(left, right);
        stream.state.set_tone_settings(*tone_settings.read());

        Ok(Self {
            data,
            metadata,
            player,
            stream,
            sample_rate,
            total_samples,
        })
    }

    /// Create audio source from a shared player (used by playback system).
    pub(crate) fn from_shared_player(
        player: SharedSongPlayer,
        metadata: Ym2149Metadata,
        total_samples: usize,
    ) -> Self {
        // Start the audio stream with the shared player
        let stream = Arc::new(AudioStream::start(Arc::clone(&player)));

        Self {
            data: Vec::new(),
            metadata,
            player,
            stream,
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

    /// Set stereo gain values (left, right)
    pub fn set_stereo_gain(&self, left: f32, right: f32) {
        self.stream.state.set_stereo_gain(left, right);
    }

    /// Set tone processing settings
    pub fn set_tone_settings(&self, settings: ToneSettings) {
        self.stream.state.set_tone_settings(settings);
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
        &["ym", "aks", "ay", "sndh"]
    }
}

/// Implement Decodable to integrate with Bevy's audio system
impl Decodable for Ym2149AudioSource {
    type DecoderItem = f32;
    type Decoder = StreamingDecoder;

    fn decoder(&self) -> Self::Decoder {
        StreamingDecoder::new(
            self.stream.shared_state(),
            self.sample_rate,
            self.total_samples,
        )
    }
}
