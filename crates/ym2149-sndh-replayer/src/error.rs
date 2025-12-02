//! Error types for SNDH parsing and playback.

use thiserror::Error;

/// Result type for SNDH operations.
pub type Result<T> = std::result::Result<T, SndhError>;

/// Errors that can occur during SNDH parsing and playback.
#[derive(Error, Debug)]
pub enum SndhError {
    /// Invalid or missing SNDH header
    #[error("Invalid SNDH header: {0}")]
    InvalidHeader(String),

    /// ICE decompression failed
    #[error("ICE decompression failed: {0}")]
    IceDepackError(String),

    /// Invalid subsong index
    #[error("Invalid subsong index {index} (available: 1-{available})")]
    InvalidSubsong {
        /// Requested subsong index
        index: usize,
        /// Number of available subsongs
        available: usize,
    },

    /// CPU execution error
    #[error("CPU execution error: {0}")]
    CpuError(String),

    /// Memory access error
    #[error("Memory access error at address 0x{address:06X}: {msg}")]
    MemoryError {
        /// Memory address that caused the error
        address: u32,
        /// Error description
        msg: String,
    },

    /// Data too short for expected format
    #[error("Data too short: expected at least {expected} bytes, got {actual}")]
    DataTooShort {
        /// Expected minimum size
        expected: usize,
        /// Actual data size
        actual: usize,
    },

    /// Initialization timed out
    #[error("Initialization timed out after {frames} frames")]
    InitTimeout {
        /// Number of frames before timeout
        frames: u32,
    },
}

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
