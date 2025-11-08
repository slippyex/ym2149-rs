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
//! # // Temporarily disabled during migration
//! # /*
//! use ym_replayer::{load_song, PlaybackController};
//! use ym2149::Ym2149;
//!
//! let data = std::fs::read("song.ym").unwrap();
//! let (mut player, summary) = load_song::<Ym2149>(&data).unwrap();
//! player.play().unwrap();
//!
//! let samples = player.generate_samples(44100);
//! # */
//! ```

#![warn(missing_docs)]

/// Error types for YM replayer operations
pub use ym2149::{Result, Ym2149Error};

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

// Export module - WAV/MP3 export (optional)
#[cfg(any(feature = "export-wav", feature = "export-mp3"))]
pub mod export;

#[cfg(any(feature = "export-wav", feature = "export-mp3"))]
pub use export::ExportConfig;
