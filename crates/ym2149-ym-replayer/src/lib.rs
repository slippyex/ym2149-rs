//! YM File Format Parser and Music Replayer
//!
//! This crate provides YM music file parsing and playback for YM2149 PSG chips.
//! It supports YM2/3/5/6 file formats, tracker modes, and various effects.
//!
//! # Features
//!
//! - YM2/3/5/6 file format parsing with LHA decompression
//! - Generic over YM2149 backend (hardware-accurate or experimental)
//! - Tracker mode support (YMT1/YMT2)
//! - Mad Max digi-drums
//! - YM6 effects (SID voice, Sync Buzzer)
//! - Optional streaming audio output
//! - Optional WAV/MP3 export
//!
//! # Example
//!
//! ```no_run
//! use ym2149_ym_replayer::{load_song, PlaybackController};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let data = std::fs::read("song.ym")?;
//! let (mut player, summary) = load_song(&data)?;
//! player.play()?;
//!
//! let samples = player.generate_samples(44100);
//! # Ok(())
//! # }
//! ```

#![warn(missing_docs)]

// Error handling
mod error;
pub use error::{ReplayerError, Result};

// Core modules
pub mod compression;
pub mod loader;
pub mod parser;

// Re-export commonly used types
pub use compression::decompress_if_needed;
pub use loader::{load_bytes, load_file};
pub use parser::{
    EffectCommand, RawParser, Ym6EffectDecoder, Ym6Parser, YmMetadata, YmParser, decode_effects_ym5,
};

// Player module - YM music playback engine
pub mod player;

// Re-export player types
pub use player::{
    CycleCounter, EffectsManager, LoadSummary, PlaybackController, PlaybackState, Player,
    TimingConfig, VblSync, Ym6Info, Ym6Player, YmFileFormat, load_song,
};
