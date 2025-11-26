//! Error types for SNDH parsing and playback.

use std::fmt;

/// Result type for SNDH operations.
pub type Result<T> = std::result::Result<T, SndhError>;

/// Errors that can occur during SNDH parsing and playback.
#[derive(Debug)]
pub enum SndhError {
    /// Invalid or missing SNDH header
    InvalidHeader(String),

    /// ICE decompression failed
    IceDepackError(String),

    /// Invalid subsong index
    InvalidSubsong {
        /// Requested subsong index
        index: usize,
        /// Number of available subsongs
        available: usize,
    },

    /// CPU execution error
    CpuError(String),

    /// Memory access error
    MemoryError {
        /// Memory address that caused the error
        address: u32,
        /// Error description
        msg: String,
    },

    /// Data too short for expected format
    DataTooShort {
        /// Expected minimum size
        expected: usize,
        /// Actual data size
        actual: usize,
    },

    /// Initialization timed out
    InitTimeout {
        /// Number of frames before timeout
        frames: u32,
    },
}

impl fmt::Display for SndhError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHeader(msg) => write!(f, "Invalid SNDH header: {msg}"),
            Self::IceDepackError(msg) => write!(f, "ICE decompression failed: {msg}"),
            Self::InvalidSubsong { index, available } => {
                write!(f, "Invalid subsong index {index} (available: 1-{available})")
            }
            Self::CpuError(msg) => write!(f, "CPU execution error: {msg}"),
            Self::MemoryError { address, msg } => {
                write!(f, "Memory access error at address 0x{address:06X}: {msg}")
            }
            Self::DataTooShort { expected, actual } => {
                write!(
                    f,
                    "Data too short: expected at least {expected} bytes, got {actual}"
                )
            }
            Self::InitTimeout { frames } => {
                write!(f, "Initialization timed out after {frames} frames")
            }
        }
    }
}

impl std::error::Error for SndhError {}

impl From<String> for SndhError {
    fn from(msg: String) -> Self {
        SndhError::CpuError(msg)
    }
}

impl From<&str> for SndhError {
    fn from(msg: &str) -> Self {
        SndhError::CpuError(msg.to_string())
    }
}
