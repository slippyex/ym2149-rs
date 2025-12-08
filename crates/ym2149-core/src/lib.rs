//! YM2149 PSG Emulator for ATARI ST
//!
//! A cycle-accurate emulator of the Yamaha YM2149 Programmable Sound Generator
//! as integrated into the ATARI ST computer.
//!
//! # Features
//! - Cycle-accurate emulation of all 3 audio channels (clk/8 internal step)
//! - Hardware envelope/volume tables (10 shapes, 32-step volume), buzzer/digidrum correct
//! - 50Hz VBL (Vertical Blanking) synchronization
//! - Raw register dump support
//! - Audio sample generation
//!
//! # Backend Trait
//! The `Ym2149Backend` trait (from `ym2149-common`) allows alternative implementations
//! (e.g., `ym2149-softsynth` crate) to be used interchangeably with the hardware-accurate backend.
//!
//! # Quick start
//! ```no_run
//! use ym2149::{Ym2149, Ym2149Backend};
//! let mut chip = Ym2149::new();
//! chip.write_register(0, 0x1C); // Tone A Lo
//! chip.write_register(1, 0x01); // Tone A Hi
//! chip.write_register(8, 0x0F); // Volume A
//! chip.clock();
//! let sample = chip.get_sample();
//! ```
//!
//! For YM file playback, use the `ym2149-ym-replayer` crate which provides YM2-YM6 format support.
//! For real-time audio streaming, use the `ym2149-replayer-cli` crate.

#![warn(missing_docs)]

// Core emulation modules
mod chip;
pub mod constants;
mod dc_filter;
mod generators;
mod mixer;
pub mod psg_bank;
mod tables;

/// Error types for YM2149 chip emulator operations
///
/// This enum only contains errors that can occur in the core chip emulation.
/// File parsing and decompression errors are handled by the `ym2149-ym-replayer` crate.
#[derive(thiserror::Error, Debug)]
pub enum Ym2149Error {
    /// IO error from filesystem or device
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Invalid configuration
    #[error("Invalid configuration: {0}")]
    ConfigError(String),

    /// Generic error
    #[error("{0}")]
    Other(String),
}

impl From<String> for Ym2149Error {
    /// Converts a String into `Ym2149Error::Other`.
    fn from(msg: String) -> Self {
        Ym2149Error::Other(msg)
    }
}

impl From<&str> for Ym2149Error {
    /// Converts a string slice into `Ym2149Error::Other`.
    fn from(msg: &str) -> Self {
        Ym2149Error::Other(msg.to_string())
    }
}

/// Result type for emulator operations
pub type Result<T> = std::result::Result<T, Ym2149Error>;

// Public API exports
pub use chip::Ym2149;
pub use constants::get_volume;
pub use psg_bank::PsgBank;
pub use ym2149_common::Ym2149Backend;
