//! YM File Loader
//!
//! Loads YM music files from disk with automatic format detection
//! and transparent decompression support.

use crate::ym_parser::FormatParser;
use crate::{compression, ym_parser, Result};
use std::fs;

/// Loads YM files from disk
pub struct YmFileLoader;

impl YmFileLoader {
    /// Load a YM file from disk, auto-detecting format and handling decompression
    pub fn load(path: &str) -> Result<Vec<[u8; 16]>> {
        // Read raw file data
        let file_data =
            fs::read(path).map_err(|e| format!("Failed to read file '{}': {}", path, e))?;

        // Transparently decompress if LHA-compressed, otherwise use as-is
        let data = compression::decompress_if_needed(&file_data)?;

        // Detect format from magic bytes
        let format = Self::detect_format(&data);

        // Parse and return frames based on detected format
        match format {
            "YM3" | "YM4" | "YM5" => {
                let parser = ym_parser::YmParser;
                parser.parse(&data)
            }
            "YM6" => {
                let parser = ym_parser::Ym6Parser;
                parser.parse(&data)
            }
            _ => Err("Unsupported file format. Supported: YM3, YM4, YM5, YM6"
                .to_string()
                .into()),
        }
    }

    /// Detect file format from magic bytes
    fn detect_format(data: &[u8]) -> &'static str {
        if data.len() < 4 {
            return "unknown";
        }

        match &data[0..4] {
            b"YM3!" => "YM3",
            b"YM4!" => "YM4",
            b"YM5!" => "YM5",
            b"YM6!" => "YM6",
            _ => "unknown",
        }
    }
}
