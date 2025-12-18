//! YM File Loader Domain
//!
//! Handles file I/O operations for loading YM music files from disk,
//! including format auto-detection and compression handling.

use crate::parser::FormatParser;
use crate::{Result, compression, parser};
use std::fs;

/// Loads YM files from disk
pub struct YmFileLoader;

impl YmFileLoader {
    /// Load a YM file from disk, auto-detecting format and handling decompression
    pub fn load(path: &str) -> Result<Vec<[u8; 16]>> {
        // Read raw file data
        let file_data = fs::read(path).map_err(|e| format!("Failed to read file '{path}': {e}"))?;

        // Load from in-memory bytes (handles decompression + parse)
        Self::load_from_bytes(&file_data)
    }

    /// Load a YM file from an in-memory byte buffer, auto-detecting format and handling decompression
    pub fn load_from_bytes(bytes: &[u8]) -> Result<Vec<[u8; 16]>> {
        // Transparently decompress if LHA-compressed, otherwise use as-is
        let data = compression::decompress_if_needed(bytes)?;

        // Detect format from magic bytes
        let format = Self::detect_format(&data);

        // Parse and return frames based on detected format
        match format {
            "YM3" | "YM3b" | "YM4" | "YM5" => {
                let parser = parser::YmParser;
                parser.parse(&data)
            }
            "YM6" => {
                let parser = parser::Ym6Parser;
                parser.parse(&data)
            }
            _ => Err(
                "Unsupported file format. Supported: YM3, YM3b, YM4, YM5, YM6"
                    .to_string()
                    .into(),
            ),
        }
    }

    /// Detect file format from magic bytes
    fn detect_format(data: &[u8]) -> &'static str {
        if data.len() < 4 {
            return "unknown";
        }

        match &data[0..4] {
            b"YM3!" => "YM3",
            b"YM3b" => "YM3b",
            b"YM4!" => "YM4",
            b"YM5!" => "YM5",
            b"YM6!" => "YM6",
            _ => "unknown",
        }
    }
}

/// Convenience function to load a YM file from disk
pub fn load_file(path: &str) -> Result<Vec<[u8; 16]>> {
    YmFileLoader::load(path)
}

/// Convenience function to load a YM file from an in-memory byte buffer
///
/// Supports automatic LHA decompression and format auto-detection.
pub fn load_bytes(data: &[u8]) -> Result<Vec<[u8; 16]>> {
    YmFileLoader::load_from_bytes(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_from_bytes_ym3_minimal() {
        // Minimal valid YM3: header + 14 bytes (one frame)
        let mut data = Vec::new();
        data.extend_from_slice(b"YM3!");
        data.extend_from_slice(&[0u8; 14]);

        let frames = YmFileLoader::load_from_bytes(&data).expect("should parse YM3 bytes");
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0][14], 0);
        assert_eq!(frames[0][15], 0);
    }

    #[test]
    fn test_detect_format_variants() {
        assert_eq!(YmFileLoader::detect_format(b"YM3!xxxx"), "YM3");
        assert_eq!(YmFileLoader::detect_format(b"YM3bxxxx"), "YM3b");
        assert_eq!(YmFileLoader::detect_format(b"YM4!xxxx"), "YM4");
        assert_eq!(YmFileLoader::detect_format(b"YM5!xxxx"), "YM5");
        assert_eq!(YmFileLoader::detect_format(b"YM6!xxxx"), "YM6");
        assert_eq!(YmFileLoader::detect_format(b"XX"), "unknown");
    }
}
