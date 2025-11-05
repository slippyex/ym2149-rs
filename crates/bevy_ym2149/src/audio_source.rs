//! YM2149 audio source asset type for Bevy

use bevy::asset::{Asset, AssetLoader, LoadContext};
use bevy::reflect::TypePath;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use ym2149::ym_loader;

use crate::error::{BevyYm2149Error, Result};

/// A loaded YM2149 audio file ready to be played
#[derive(Asset, TypePath, Clone)]
pub struct Ym2149AudioSource {
    /// The raw YM file data
    pub data: Vec<u8>,
    /// Cached metadata about the YM file
    pub metadata: Ym2149Metadata,
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
        // Extract metadata
        let metadata = extract_metadata(&data)?;

        Ok(Self { data, metadata })
    }

    /// Get the duration of this audio source in seconds
    pub fn duration(&self) -> f32 {
        self.metadata.duration_seconds
    }

    /// Get metadata for this audio source
    pub fn metadata(&self) -> &Ym2149Metadata {
        &self.metadata
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
        &["ym"]
    }
}

/// Extract metadata from YM file data
fn extract_metadata(data: &[u8]) -> Result<Ym2149Metadata> {
    let _frames = ym_loader::load_bytes(data).map_err(|e| {
        BevyYm2149Error::MetadataExtraction(format!("Failed to load frames: {}", e))
    })?;

    // Use load_song to get the player with metadata
    let (player, summary) = ym2149::load_song(data)
        .map_err(|e| BevyYm2149Error::MetadataExtraction(format!("Failed to load song: {}", e)))?;

    let frame_count = summary.frame_count;

    // Try to get metadata from player's info
    let (title, author, comment) = if let Some(info) = player.info() {
        (
            info.song_name.clone(),
            info.author.clone(),
            info.comment.clone(),
        )
    } else {
        (String::new(), String::new(), String::new())
    };

    // Calculate duration: samples = frame_count * samples_per_frame
    // At 44.1kHz, samples_per_frame is typically 882
    let samples_per_frame = summary.samples_per_frame as f32;
    let total_samples = frame_count as f32 * samples_per_frame;
    let duration_seconds = total_samples / 44100.0; // Standard 44.1kHz

    Ok(Ym2149Metadata {
        title,
        author,
        comment,
        frame_count,
        duration_seconds,
    })
}
