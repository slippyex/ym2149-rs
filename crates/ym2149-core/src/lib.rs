//! YM2149 PSG Emulator for ATARI ST
//!
//! A cycle-accurate emulator of the Yamaha YM2149 Programmable Sound Generator
//! as integrated into the ATARI ST computer. Supports MFP timer integration,
//! VBL synchronization, and playback of YM chiptune files.
//!
//! # Features
//! - Cycle-accurate emulation of all 3 audio channels
//! - Full envelope generator support
//! - MFP Timer A/B/C integration for modulation effects
//! - 50Hz VBL (Vertical Blanking) synchronization
//! - YM file format parser and playback
//! - Raw register dump support
//! - Audio sample generation and optional streaming playback
//!
//! # Crate feature flags
//! - `emulator` (default): Core YM2149 cycle-accurate emulator
//! - `streaming` (optional): Real-time audio output via rodio (for CLI tools)
//! - `visualization` (optional): Terminal visualization helpers (for CLI tools)
//!
//! # Backend Trait
//! The `Ym2149Backend` trait allows alternative implementations (e.g., `ym-softsynth` crate)
//! to be used interchangeably with the hardware-accurate emulation.
//!
//! # Quick start
//! ## Core emulator only
//! ```no_run
//! use ym2149::ym2149::Ym2149;
//! let mut chip = Ym2149::new();
//! chip.write_register(0, 0x1C); // Tone A Lo
//! chip.write_register(1, 0x01); // Tone A Hi
//! chip.write_register(8, 0x0F); // Volume A
//! chip.clock();
//! let sample = chip.get_sample();
//! ```
//!
//! ## Real-time streaming with chip demo
//! ```no_run
//! # #[cfg(feature = "streaming")]
//! # {
//! use ym2149::{Ym2149, RealtimePlayer, StreamConfig, AudioDevice};
//! use std::sync::Arc;
//!
//! // Create chip and configure for A4 tone
//! let mut chip = Ym2149::new();
//! chip.write_register(0x07, 0x3E); // Enable tone A
//! chip.write_register(0x00, 0x1C); // A4 period low
//! chip.write_register(0x01, 0x01); // A4 period high
//! chip.write_register(0x08, 0x0F); // Max volume
//!
//! // Setup audio streaming
//! let cfg = StreamConfig::default();
//! let stream = RealtimePlayer::new(cfg).unwrap();
//! let _dev = AudioDevice::new(cfg.sample_rate, cfg.channels, stream.get_buffer()).unwrap();
//! # }
//! ```
//!
//! For YM file playback, use the `ym-replayer` crate which provides YM2-YM6 format support.

#![warn(missing_docs)]

// Domain modules (feature-gated for modular use)
pub mod backend; // Backend trait abstraction
pub mod mfp;
pub mod util;
pub mod ym2149; // YM2149 PSG Emulation (core) // MFP Timer Effects (helpers) // Shared helper utilities

#[cfg(feature = "streaming")]
pub mod streaming; // Audio Output & Streaming

#[cfg(feature = "visualization")]
pub mod visualization; // Terminal UI Helpers

/// Error types for YM2149 chip emulator operations
///
/// This enum only contains errors that can occur in the core chip emulation.
/// File parsing and decompression errors are handled by the `ym-replayer` crate.
#[derive(thiserror::Error, Debug)]
pub enum Ym2149Error {
    /// IO error from filesystem or device
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Audio device error
    #[error("Audio device error: {0}")]
    AudioDeviceError(String),

    /// Invalid configuration
    #[error("Invalid configuration: {0}")]
    ConfigError(String),

    /// Generic error
    #[error("{0}")]
    Other(String),
}

impl From<String> for Ym2149Error {
    /// Converts a String into `Ym2149Error::Other`.
    ///
    /// This is a convenience conversion for generic string errors. Note that all string errors
    /// are converted to the `Other` variant, losing semantic information about the error type.
    ///
    /// For better error discrimination, use specific variant constructors instead:
    /// - `Ym2149Error::ParseError(msg)` for file format parsing failures
    /// - `Ym2149Error::ConfigError(msg)` for invalid configuration
    /// - `Ym2149Error::AudioFileError(msg)` for audio output issues
    /// - `Ym2149Error::AudioDeviceError(msg)` for device initialization
    /// - `Ym2149Error::DecompressionError(msg)` for decompression failures
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Discouraged: loses error type information
    /// return Err(format!("Invalid parameter").into());
    ///
    /// // Preferred: preserves error discrimination
    /// return Err(Ym2149Error::ConfigError(format!("Invalid parameter")));
    /// ```
    fn from(msg: String) -> Self {
        Ym2149Error::Other(msg)
    }
}

impl From<&str> for Ym2149Error {
    /// Converts a string slice into `Ym2149Error::Other`.
    ///
    /// This is a convenience conversion for generic string errors. See [`From<String>`]
    /// for guidance on when to use explicit variant constructors instead.
    fn from(msg: &str) -> Self {
        Ym2149Error::Other(msg.to_string())
    }
}

/// Result type for emulator operations
pub type Result<T> = std::result::Result<T, Ym2149Error>;

// Public API exports
pub use backend::Ym2149Backend;
pub use mfp::Mfp;
pub use ym2149::Ym2149;

#[cfg(feature = "streaming")]
pub use streaming::{AudioDevice, RealtimePlayer, RingBuffer, StreamConfig};

#[cfg(feature = "visualization")]
pub use visualization::create_volume_bar;
