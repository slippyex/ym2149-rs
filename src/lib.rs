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
//! - `emulator` (default): Core YM2149 integer-accurate emulator (`ym2149`)
//! - `ym-format` (default): YM file parsing/loader (`ym_parser`, `ym_loader`, `compression`)
//! - `replayer` (default): YM replayer and effects decoding (`replayer`)
//! - `visualization` (default): Terminal visualization helpers (`visualization`)
//! - `streaming` (opt-in): Real-time audio output (enables optional `rodio` dep)
//! - `softsynth` (opt-in): Experimental software synthesizer (`softsynth`)
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
//! ## Load and play YM (no streaming)
//! ```no_run
//! # #[cfg(feature = "replayer")]
//! # {
//! use ym2149::replayer::PlaybackController;
//! use ym2149::{load_song, Ym6Player};
//! let data = std::fs::read("song.ym").unwrap();
//! let (mut player, summary) = load_song(&data).unwrap();
//! player.play().unwrap();
//! let audio = player.generate_samples(summary.samples_per_frame as usize);
//! # }
//! ```
//!
//! ## Real-time streaming
//! ```no_run
//! # #[cfg(all(feature = "replayer", feature = "streaming"))]
//! # {
//! use ym2149::replayer::PlaybackController;
//! use ym2149::{load_song, RealtimePlayer, StreamConfig, AudioDevice};
//! let data = std::fs::read("song.ym").unwrap();
//! let (mut player, summary) = load_song(&data).unwrap();
//! player.play().unwrap();
//! let cfg = StreamConfig::low_latency(44_100);
//! let stream = RealtimePlayer::new(cfg).unwrap();
//! let _dev = AudioDevice::new(cfg.sample_rate, cfg.channels, stream.get_buffer()).unwrap();
//! // push samples into the stream in a loop
//! # }
//! ```

#![warn(missing_docs)]

// Domain modules (feature-gated for modular use)
pub mod ym2149; // YM2149 PSG Emulation (core)

#[cfg(feature = "ym-format")]
pub mod compression; // Data Decompression (LHA/LZH)
pub mod mfp; // MFP Timer Effects (helpers)
#[cfg(feature = "replayer")]
pub mod replayer; // Playback Engine
#[cfg(feature = "softsynth")]
/// Experimental software synthesizer (non-bit-accurate)
pub mod softsynth;
#[cfg(feature = "streaming")]
pub mod streaming; // Audio Output & Streaming
#[cfg(feature = "visualization")]
pub mod visualization; // Terminal UI Helpers
#[cfg(feature = "ym-format")]
pub mod ym_loader; // YM File I/O
#[cfg(feature = "ym-format")]
pub mod ym_parser; // YM Format Parsing

/// Error types for YM2149 emulator operations
#[derive(thiserror::Error, Debug)]
pub enum Ym2149Error {
    /// Error while parsing file format
    #[error("Parse error: {0}")]
    ParseError(String),

    /// Error writing audio file
    #[error("Audio file write error: {0}")]
    AudioFileError(String),

    /// IO error from filesystem or device
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Decompression error
    #[error("Decompression error: {0}")]
    DecompressionError(String),

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
pub use ym2149::Ym2149;

#[cfg(feature = "ym-format")]
pub use compression::decompress_if_needed;
pub use mfp::Mfp;
#[cfg(feature = "replayer")]
pub use replayer::{load_song, LoadSummary, Player, Ym6Info, Ym6Player, YmFileFormat};
#[cfg(feature = "softsynth")]
pub use softsynth::SoftPlayer;
#[cfg(feature = "streaming")]
pub use streaming::{AudioDevice, RealtimePlayer, RingBuffer, StreamConfig};
#[cfg(feature = "visualization")]
pub use visualization::create_volume_bar;
#[cfg(feature = "ym-format")]
pub use ym_parser::effects::{decode_effects_ym5, EffectCommand, Ym6EffectDecoder};
