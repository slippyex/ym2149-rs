//! Error handling for AY replayer components.

use thiserror::Error;

/// Convenient result alias for AY parsing and playback.
pub type Result<T> = std::result::Result<T, AyError>;

/// Errors that may occur while parsing or replaying AY files.
#[derive(Debug, Error)]
pub enum AyError {
    /// File does not start with the expected `ZXAY` marker.
    #[error("AY file must start with ZXAY header")]
    InvalidFileId,
    /// Unsupported AY container type (only `EMUL` is recognized).
    #[error("unsupported AY type '{typ}'")]
    UnsupportedType {
        /// Type identifier encountered inside the header.
        typ: String,
    },
    /// Buffer too small to contain the requested structure.
    #[error("unexpected end of file")]
    UnexpectedEof,
    /// A required relative pointer is missing or zero.
    #[error("missing pointer at offset 0x{offset:04x}")]
    MissingPointer {
        /// Offset of the pointer field inside the file.
        offset: usize,
    },
    /// Relative pointer points outside of the file range.
    #[error("pointer at offset 0x{offset:04x} points outside AY file")]
    PointerOutOfRange {
        /// Offset of the pointer field inside the file.
        offset: usize,
    },
    /// Null-terminated string was not properly terminated before EOF.
    #[error("unterminated string at offset 0x{start:04x}")]
    UnterminatedString {
        /// Start offset for the unterminated string.
        start: usize,
    },
    /// Block table reached EOF before encountering a terminator.
    #[error("unterminated block table at offset 0x{offset:04x}")]
    UnterminatedBlockTable {
        /// Offset where block parsing stopped.
        offset: usize,
    },
    /// Generic validation error.
    #[error("{msg}")]
    InvalidData {
        /// Human-readable explanation of the validation failure.
        msg: String,
    },
}

impl From<String> for AyError {
    fn from(s: String) -> Self {
        AyError::InvalidData { msg: s }
    }
}

impl From<&str> for AyError {
    fn from(s: &str) -> Self {
        AyError::InvalidData { msg: s.to_string() }
    }
}
