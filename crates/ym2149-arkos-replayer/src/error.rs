//! Error types for Arkos Tracker parsing and playback

use std::fmt;

/// Result type for Arkos operations
pub type Result<T> = std::result::Result<T, ArkosError>;

/// Errors that can occur when loading or playing Arkos Tracker files
#[derive(Debug)]
pub enum ArkosError {
    /// XML parsing error
    XmlError(String),

    /// Invalid file format
    InvalidFormat(String),

    /// Missing required element or attribute
    MissingElement(String),

    /// Invalid value for a field
    InvalidValue {
        /// Field name
        field: String,
        /// Invalid value
        value: String,
        /// Expected format
        expected: String,
    },

    /// Subsong index out of range
    InvalidSubsong {
        /// Requested index
        index: usize,
        /// Available subsongs
        available: usize,
    },

    /// PSG configuration error
    PsgError(String),

    /// I/O error
    IoError(std::io::Error),
}

impl fmt::Display for ArkosError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArkosError::XmlError(msg) => write!(f, "XML parsing error: {}", msg),
            ArkosError::InvalidFormat(msg) => write!(f, "Invalid AKS format: {}", msg),
            ArkosError::MissingElement(elem) => write!(f, "Missing required element: {}", elem),
            ArkosError::InvalidValue {
                field,
                value,
                expected,
            } => write!(
                f,
                "Invalid value for '{}': got '{}', expected {}",
                field, value, expected
            ),
            ArkosError::InvalidSubsong { index, available } => {
                write!(f, "Subsong {} out of range (0..{})", index, available)
            }
            ArkosError::PsgError(msg) => write!(f, "PSG error: {}", msg),
            ArkosError::IoError(err) => write!(f, "I/O error: {}", err),
        }
    }
}

impl std::error::Error for ArkosError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ArkosError::IoError(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for ArkosError {
    fn from(err: std::io::Error) -> Self {
        ArkosError::IoError(err)
    }
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
