//! Unified playback metadata trait.
//!
//! This module provides a common interface for metadata across different
//! chiptune file formats (YM, AKS, AY).

/// Unified metadata trait for chiptune playback.
///
/// Implementations of this trait provide a common interface to access
/// song metadata regardless of the underlying file format.
pub trait MetadataFields {
    /// Get the song title.
    fn title(&self) -> &str;

    /// Get the author/composer name.
    fn author(&self) -> &str;

    /// Get additional comments or description.
    ///
    /// Returns an empty string if no comments are available.
    fn comments(&self) -> &str {
        ""
    }

    /// Get the file format identifier.
    ///
    /// Examples: "YM6", "AKS", "AY"
    fn format(&self) -> &str;

    /// Get the total frame count, if known.
    fn frame_count(&self) -> Option<usize> {
        None
    }

    /// Get the playback frame rate in Hz.
    ///
    /// Typical values: 50 (PAL) or 60 (NTSC).
    fn frame_rate(&self) -> u32 {
        50
    }

    /// Get the song duration in seconds, if known.
    fn duration_seconds(&self) -> Option<f32> {
        self.frame_count()
            .map(|fc| fc as f32 / self.frame_rate() as f32)
    }

    /// Get the loop start frame, if the song loops.
    fn loop_frame(&self) -> Option<usize> {
        None
    }
}

/// Unified metadata trait for chiptune playback.
///
/// This is a thin marker over [`MetadataFields`], provided for compatibility
/// with the existing public API.
pub trait PlaybackMetadata: MetadataFields {}

impl<T: MetadataFields> PlaybackMetadata for T {}

/// Basic metadata container implementing `PlaybackMetadata`.
///
/// This is a simple struct that can be used when you need to store
/// metadata without the original parser structures.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BasicMetadata {
    /// Song title.
    pub title: String,
    /// Author/composer name.
    pub author: String,
    /// Additional comments.
    pub comments: String,
    /// File format identifier (e.g., "YM6", "AKS").
    pub format: String,
    /// Total frame count.
    pub frame_count: Option<usize>,
    /// Playback frame rate in Hz.
    pub frame_rate: u32,
    /// Loop start frame.
    pub loop_frame: Option<usize>,
}

impl MetadataFields for BasicMetadata {
    fn title(&self) -> &str {
        &self.title
    }

    fn author(&self) -> &str {
        &self.author
    }

    fn comments(&self) -> &str {
        &self.comments
    }

    fn format(&self) -> &str {
        &self.format
    }

    fn frame_count(&self) -> Option<usize> {
        self.frame_count
    }

    fn frame_rate(&self) -> u32 {
        self.frame_rate
    }

    fn loop_frame(&self) -> Option<usize> {
        self.loop_frame
    }
}

impl BasicMetadata {
    /// Create a new `BasicMetadata` with default values.
    pub fn new() -> Self {
        Self {
            frame_rate: 50,
            ..Default::default()
        }
    }

    /// Create metadata from title and author.
    pub fn with_title_author(title: impl Into<String>, author: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            author: author.into(),
            frame_rate: 50,
            ..Default::default()
        }
    }
}
