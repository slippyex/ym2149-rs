//! # ym2149-sndh-replayer
//!
//! SNDH file parser and Atari ST machine emulation for YM2149 chiptune playback.
//!
//! This crate provides playback support for SNDH files, a popular format for
//! Atari ST chiptune music. It includes:
//!
//! - **SNDH Parser**: Parses SNDH file headers and metadata
//! - **ICE Depacker**: Decompresses ICE! 2.4 packed SNDH files
//! - **68000 CPU Emulation**: Via the `m68000` crate for executing SNDH drivers
//! - **MFP68901 Timer Emulation**: For accurate timer-based effects (SID voice, etc.)
//! - **Atari ST Machine**: Memory-mapped I/O emulation for YM2149 and timers
//!
//! ## Example
//!
//! ```rust,ignore
//! use ym2149_sndh_replayer::{SndhPlayer, load_sndh};
//! use ym2149_common::ChiptunePlayer;
//!
//! let data = std::fs::read("music.sndh")?;
//! let mut player = load_sndh(&data, 44100)?;
//!
//! // Select subsong (1-based index)
//! player.init_subsong(1)?;
//! player.play();
//!
//! // Generate audio samples
//! let mut buffer = vec![0.0f32; 882]; // ~50Hz at 44100 sample rate
//! player.generate_samples_into(&mut buffer);
//! ```
//!
//! ## SNDH Format
//!
//! SNDH is a standard format for Atari ST music that embeds the original
//! 68000 replay code along with the music data. The format uses a simple
//! header followed by executable code:
//!
//! - Entry point + 0: Initialize subsong (D0 = subsong number)
//! - Entry point + 4: Exit/cleanup
//! - Entry point + 8: Play one frame (called at player rate, typically 50Hz)
//!
//! Many SNDH files are ICE! packed for smaller file sizes.

#![warn(missing_docs)]

mod error;
mod ice;
mod machine;
mod mfp68901;
mod parser;
mod player;

pub use error::{Result, SndhError};
pub use ice::{ice_depack, is_ice_packed};
pub use parser::{SndhFile, SndhMetadata, SubsongInfo};
pub use player::SndhPlayer;

// Re-export common traits for convenience
pub use ym2149_common::{BasicMetadata, ChiptunePlayer, PlaybackMetadata, PlaybackState};

/// Check if data appears to be SNDH format.
///
/// This performs a quick header check without fully parsing the file.
/// It handles both raw SNDH and ICE!-packed SNDH files.
///
/// # Arguments
///
/// * `data` - Raw file data to check
///
/// # Returns
///
/// `true` if the data appears to be SNDH format.
pub fn is_sndh_data(data: &[u8]) -> bool {
    // Check for ICE! packed data first
    if is_ice_packed(data) {
        // For ICE! packed files, we can't easily check the header
        // but ICE! is commonly used for SNDH, so we'll accept it
        // The actual validation happens during parsing
        return true;
    }

    // Check minimum size for SNDH header
    if data.len() < 16 {
        return false;
    }

    // Check for BRA instruction at offset 0
    if data[0] != 0x60 {
        return false;
    }

    // Check for SNDH magic at offset 12
    &data[12..16] == b"SNDH"
}

/// Load an SNDH file and create a player.
///
/// This is the main entry point for playing SNDH files. It handles:
/// - ICE! decompression if needed
/// - SNDH header parsing
/// - Atari ST machine initialization
///
/// # Arguments
///
/// * `data` - Raw SNDH file data (may be ICE! compressed)
/// * `sample_rate` - Output sample rate (e.g., 44100)
///
/// # Returns
///
/// A configured `SndhPlayer` ready for playback.
pub fn load_sndh(data: &[u8], sample_rate: u32) -> Result<SndhPlayer> {
    SndhPlayer::new(data, sample_rate)
}
