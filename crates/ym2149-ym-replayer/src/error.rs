//! Error types for YM file parsing and playback

use ym2149::Ym2149Error;

/// Error type for YM file replayer operations
#[derive(thiserror::Error, Debug)]
pub enum ReplayerError {
    /// Error while parsing YM file format
    #[error("Parse error: {0}")]
    ParseError(String),

    /// Decompression error (LHA/LZH)
    #[error("Decompression error: {0}")]
    DecompressionError(String),

    /// IO error from filesystem
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Error from underlying chip emulation
    #[error("Chip error: {0}")]
    ChipError(#[from] Ym2149Error),

    /// Invalid configuration
    #[error("Invalid configuration: {0}")]
    ConfigError(String),

    /// Generic error
    #[error("{0}")]
    Other(String),
}

impl From<String> for ReplayerError {
    fn from(s: String) -> Self {
        ReplayerError::Other(s)
    }
}

impl From<&str> for ReplayerError {
    fn from(s: &str) -> Self {
        ReplayerError::Other(s.to_string())
    }
}

/// Result type for replayer operations
pub type Result<T> = std::result::Result<T, ReplayerError>;
