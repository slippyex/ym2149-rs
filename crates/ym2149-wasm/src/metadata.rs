//! Metadata types for WebAssembly player.
//!
//! This module provides the `YmMetadata` struct exposed to JavaScript
//! and conversion functions from various player metadata formats.

use wasm_bindgen::prelude::*;
use ym2149_ay_replayer::AyMetadata as AyFileMetadata;
use ym2149_ym_replayer::LoadSummary;

/// YM file metadata exposed to JavaScript.
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct YmMetadata {
    pub(crate) title: String,
    pub(crate) author: String,
    pub(crate) comments: String,
    pub(crate) format: String,
    pub(crate) frame_count: u32,
    pub(crate) frame_rate: u32,
    pub(crate) duration_seconds: f32,
}

#[wasm_bindgen]
impl YmMetadata {
    /// Get the song title.
    #[wasm_bindgen(getter)]
    pub fn title(&self) -> String {
        self.title.clone()
    }

    /// Get the song author.
    #[wasm_bindgen(getter)]
    pub fn author(&self) -> String {
        self.author.clone()
    }

    /// Get the song comments.
    #[wasm_bindgen(getter)]
    pub fn comments(&self) -> String {
        self.comments.clone()
    }

    /// Get the YM format version.
    #[wasm_bindgen(getter)]
    pub fn format(&self) -> String {
        self.format.clone()
    }

    /// Get frame count.
    #[wasm_bindgen(getter)]
    pub fn frame_count(&self) -> u32 {
        self.frame_count
    }

    /// Get frame rate in Hz.
    #[wasm_bindgen(getter)]
    pub fn frame_rate(&self) -> u32 {
        self.frame_rate
    }

    /// Get duration in seconds.
    #[wasm_bindgen(getter)]
    pub fn duration_seconds(&self) -> f32 {
        self.duration_seconds
    }
}

/// Convert YM player info to metadata.
pub fn metadata_from_summary(
    player: &ym2149_ym_replayer::YmPlayer,
    summary: &LoadSummary,
) -> YmMetadata {
    let (title, author, comments, frame_rate) = if let Some(info) = player.info() {
        (
            info.song_name.clone(),
            info.author.clone(),
            info.comment.clone(),
            info.frame_rate as u32,
        )
    } else {
        (
            "Unknown".to_string(),
            "Unknown".to_string(),
            String::new(),
            50u32,
        )
    };

    YmMetadata {
        title,
        author,
        comments,
        format: format!("{:?}", summary.format),
        frame_count: summary.frame_count as u32,
        frame_rate,
        duration_seconds: player.get_duration_seconds(),
    }
}

/// Convert AY file metadata to common metadata format.
pub fn metadata_from_ay(meta: &AyFileMetadata) -> YmMetadata {
    let frame_count = meta.frame_count.unwrap_or(0);
    let duration_seconds = meta
        .duration_seconds
        .unwrap_or_else(|| frame_count as f32 / 50.0);

    YmMetadata {
        title: meta.song_name.clone(),
        author: meta.author.clone(),
        comments: meta.misc.clone(),
        format: "AY".to_string(),
        frame_count: frame_count as u32,
        frame_rate: 50,
        duration_seconds,
    }
}
