//! Error types for Arkos Tracker parsing and playback.

use thiserror::Error;

/// Result type for Arkos operations.
pub type Result<T> = std::result::Result<T, ArkosError>;

/// Errors that can occur when loading or playing Arkos Tracker files.
#[derive(Error, Debug)]
pub enum ArkosError {
    /// XML parsing error.
    #[error("XML parsing error: {0}")]
    XmlError(String),

    /// Invalid file format.
    #[error("Invalid AKS format: {0}")]
    InvalidFormat(String),

    /// Missing required element or attribute.
    #[error("Missing required element: {0}")]
    MissingElement(String),

    /// Invalid value for a field.
    #[error("Invalid value for '{field}': got '{value}', expected {expected}")]
    InvalidValue {
        /// Field name.
        field: String,
        /// Invalid value.
        value: String,
        /// Expected format.
        expected: String,
    },

    /// Subsong index out of range.
    #[error("Subsong {index} out of range (0..{available})")]
    InvalidSubsong {
        /// Requested index.
        index: usize,
        /// Available subsongs.
        available: usize,
    },

    /// PSG configuration error.
    #[error("PSG error: {0}")]
    PsgError(String),

    /// I/O error.
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
}

impl From<quick_xml::Error> for ArkosError {
    fn from(err: quick_xml::Error) -> Self {
        ArkosError::XmlError(err.to_string())
    }
}

impl From<std::num::ParseIntError> for ArkosError {
    fn from(err: std::num::ParseIntError) -> Self {
        ArkosError::InvalidValue {
            field: "integer".to_string(),
            value: err.to_string(),
            expected: "valid integer".to_string(),
        }
    }
}

impl From<std::num::ParseFloatError> for ArkosError {
    fn from(err: std::num::ParseFloatError) -> Self {
        ArkosError::InvalidValue {
            field: "float".to_string(),
            value: err.to_string(),
            expected: "valid float".to_string(),
        }
    }
}

impl From<String> for ArkosError {
    fn from(s: String) -> Self {
        ArkosError::InvalidFormat(s)
    }
}

impl From<&str> for ArkosError {
    fn from(s: &str) -> Self {
        ArkosError::InvalidFormat(s.to_string())
    }
}
