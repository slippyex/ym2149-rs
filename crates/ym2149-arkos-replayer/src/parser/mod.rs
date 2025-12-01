//! AKS XML parser for Arkos Tracker 2/3 files.
//!
//! This module provides comprehensive parsing for Arkos Tracker song files (`.aks`),
//! supporting both legacy Format 1.x (Arkos Tracker 2) and modern Format 3.x
//! (Arkos Tracker 3) XML structures.
//!
//! # File Format Support
//!
//! AKS files can be either:
//! - **Plain XML**: Uncompressed XML text (typically from test exports)
//! - **ZIP-compressed**: Production files with a single `.aks` XML inside
//!
//! The parser automatically detects the format based on file magic bytes.
//!
//! # Architecture
//!
//! The parser is organized into internal submodules:
//!
//! - Parse state machine and builder types
//! - XML parsing utilities (position blocks, skip helpers)
//! - Core XML parsing logic
//!
//! # Example
//!
//! ```no_run
//! use ym2149_arkos_replayer::parser::load_aks;
//!
//! let data = std::fs::read("song.aks")?;
//! let song = load_aks(&data)?;
//!
//! println!("Title: {}", song.metadata.title);
//! println!("Subsongs: {}", song.subsongs.len());
//! for (i, subsong) in song.subsongs.iter().enumerate() {
//!     println!("  Subsong {}: {} - {} patterns, {} PSG chips",
//!         i, subsong.title, subsong.patterns.len(), subsong.psgs.len());
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Format Differences
//!
//! ## Legacy Format (1.x)
//! - Uses `<fmInstrument>` instead of `<instrument>`
//! - Arpeggios stored with `<arpeggioCell>` containing `<note>` and `<octave>`
//! - Effects use `<effectAndValue>` with `<hexValue>` requiring decoding
//! - Patterns implicitly create positions (no separate positions block)
//!
//! ## Modern Format (3.x)
//! - Uses `<instrument>` with `<cells>` containing `<cell>` elements
//! - Arpeggios use `<expression>` with direct `<value>` entries
//! - Effects use `<effect>` with `<logicalValue>`
//! - Explicit `<positions>` block separate from patterns

mod helpers;
mod state;
mod xml_parser;

#[cfg(test)]
mod tests;

// Re-export public API
pub use helpers::{parse_positions_block, skip_block};
pub use state::{DEFAULT_DIGIDRUM_NOTE, FormatVersion, ParseState, SampleInstrumentBuilder};

use crate::error::{ArkosError, Result};
use crate::format::AksSong;

/// Loads an AKS file from bytes, auto-detecting format.
///
/// Automatically detects whether the file is:
/// - **Plain XML**: Test files or uncompressed exports
/// - **ZIP-compressed**: Production/packaged files (magic bytes: `PK\x03\x04`)
///
/// # Arguments
///
/// * `data` - Raw AKS file bytes (XML or ZIP)
///
/// # Returns
///
/// Parsed [`AksSong`] containing all song data including:
/// - Metadata (title, author, composer, comments)
/// - Instruments with PSG/digi configuration
/// - Arpeggios and pitch tables
/// - Subsongs with patterns, tracks, and positions
///
/// # Errors
///
/// Returns [`ArkosError`] if:
/// - ZIP extraction fails
/// - XML parsing fails
/// - Required elements are missing
/// - Data validation fails
///
/// # Example
///
/// ```no_run
/// use ym2149_arkos_replayer::load_aks;
///
/// // Load from file
/// let data = std::fs::read("song.aks")?;
/// let song = load_aks(&data)?;
///
/// println!("Title: {}", song.metadata.title);
/// println!("Subsongs: {}", song.subsongs.len());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn load_aks(data: &[u8]) -> Result<AksSong> {
    // Check if it's a ZIP file (magic bytes: PK\x03\x04)
    if data.len() >= 4 && &data[0..2] == b"PK" {
        return load_aks_zip(data);
    }

    // Plain XML AKS file
    xml_parser::parse_aks_xml(data)
}

/// Loads a ZIP-compressed AKS file.
///
/// AKS files from Arkos Tracker are typically saved as ZIP archives
/// containing a single XML file with the same base name.
///
/// # Arguments
///
/// * `data` - Raw ZIP file bytes
///
/// # Errors
///
/// Returns [`ArkosError::InvalidFormat`] if:
/// - Not a valid ZIP file
/// - ZIP contains more or fewer than 1 file
/// - Contained file cannot be read
fn load_aks_zip(data: &[u8]) -> Result<AksSong> {
    use std::io::{Cursor, Read};
    use zip::ZipArchive;

    let cursor = Cursor::new(data);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|e| ArkosError::InvalidFormat(format!("Not a valid ZIP file: {}", e)))?;

    // AKS ZIP files contain a single .aks file with the same name
    if archive.len() != 1 {
        return Err(ArkosError::InvalidFormat(format!(
            "Expected 1 file in ZIP, found {}",
            archive.len()
        )));
    }

    let mut file = archive
        .by_index(0)
        .map_err(|e| ArkosError::InvalidFormat(format!("Cannot read ZIP entry: {}", e)))?;

    let mut xml_data = Vec::new();
    file.read_to_end(&mut xml_data)
        .map_err(ArkosError::IoError)?;

    xml_parser::parse_aks_xml(&xml_data)
}
