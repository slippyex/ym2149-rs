//! YM File Loader Domain
//!
//! Handles file I/O operations for loading YM music files from disk,
//! including format auto-detection and compression handling.

pub mod loader;

pub use loader::YmFileLoader;

use crate::Result;

/// Convenience function to load a YM file from disk
pub fn load_file(path: &str) -> Result<Vec<[u8; 16]>> {
    YmFileLoader::load(path)
}
