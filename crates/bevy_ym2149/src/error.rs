//! Error types for the bevy_ym2149 plugin
//!
//! This module provides comprehensive error handling for YM2149 audio playback,
//! file loading, and audio device management.

use thiserror::Error;

/// The main error type for bevy_ym2149 operations
///
/// This enum represents all possible errors that can occur during YM file loading,
/// playback, and audio device management in the bevy_ym2149 plugin.
#[derive(Error, Debug)]
pub enum BevyYm2149Error {
    /// Error reading a file from disk
    #[error("Failed to read file '{path}': {reason}")]
    FileRead { path: String, reason: String },

    /// File not found
    #[error("File not found: {0}")]
    FileNotFound(String),

    /// Invalid file format or corrupted file
    #[error("Invalid YM file format: {0}")]
    InvalidFormat(String),

    /// Error initializing the YM2149 player/emulator
    #[error("Failed to initialize YM2149 player: {0}")]
    PlayerInitialization(String),

    /// Error during playback operations
    #[error("Playback error: {0}")]
    PlayerPlayback(String),

    /// Error with player state management
    #[error("Invalid player state: {0}")]
    PlayerState(String),

    /// Error creating audio output device
    #[error("Failed to create audio device: {0}")]
    AudioDeviceCreation(String),

    /// Error starting audio output device
    #[error("Failed to start audio device: {0}")]
    AudioDeviceStart(String),

    /// Error creating ring buffer for audio
    #[error("Failed to create ring buffer: {0}")]
    RingBufferCreation(String),

    /// Error pushing audio samples to device
    #[error("Failed to push audio samples: {0}")]
    SamplePush(String),

    /// Error extracting metadata from YM file
    #[error("Failed to extract metadata: {0}")]
    MetadataExtraction(String),

    /// Error loading asset
    #[error("Failed to load asset: {0}")]
    AssetLoad(String),

    /// Error initializing audio bridge
    #[error("Failed to initialize audio bridge: {0}")]
    BridgeInitialization(String),

    /// Generic or miscellaneous error
    #[error("{0}")]
    Other(String),
}

impl BevyYm2149Error {
    /// Creates a file read error with path and reason
    pub fn file_read(path: impl Into<String>, reason: impl Into<String>) -> Self {
        BevyYm2149Error::FileRead {
            path: path.into(),
            reason: reason.into(),
        }
    }

    /// Creates a file not found error
    pub fn file_not_found(path: impl Into<String>) -> Self {
        BevyYm2149Error::FileNotFound(path.into())
    }

    /// Creates an invalid format error
    pub fn invalid_format(reason: impl Into<String>) -> Self {
        BevyYm2149Error::InvalidFormat(reason.into())
    }

    /// Creates a player initialization error
    pub fn player_init(reason: impl Into<String>) -> Self {
        BevyYm2149Error::PlayerInitialization(reason.into())
    }

    /// Creates a playback error
    pub fn playback(reason: impl Into<String>) -> Self {
        BevyYm2149Error::PlayerPlayback(reason.into())
    }

    /// Creates an audio device creation error
    pub fn audio_device(reason: impl Into<String>) -> Self {
        BevyYm2149Error::AudioDeviceCreation(reason.into())
    }

    /// Creates a ring buffer creation error
    pub fn ring_buffer(reason: impl Into<String>) -> Self {
        BevyYm2149Error::RingBufferCreation(reason.into())
    }

    /// Creates a sample push error
    pub fn sample_push(reason: impl Into<String>) -> Self {
        BevyYm2149Error::SamplePush(reason.into())
    }
}

/// Type alias for Result using BevyYm2149Error
pub type Result<T> = std::result::Result<T, BevyYm2149Error>;

impl From<String> for BevyYm2149Error {
    fn from(s: String) -> Self {
        BevyYm2149Error::Other(s)
    }
}

impl From<&str> for BevyYm2149Error {
    fn from(s: &str) -> Self {
        BevyYm2149Error::Other(s.to_string())
    }
}
