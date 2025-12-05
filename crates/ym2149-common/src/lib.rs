//! Common traits and types for YM2149 chiptune replayers.
//!
//! This crate provides shared abstractions used across multiple replayer
//! implementations (YM, AKS, AY formats).
//!
//! # Traits
//!
//! - [`ChiptunePlayer`] - Unified player interface for any chiptune format
//! - [`PlaybackMetadata`] - Metadata access (title, author, duration, etc.)
//!
//! # Example
//!
//! ```ignore
//! use ym2149_common::{ChiptunePlayer, PlaybackMetadata, PlaybackState};
//!
//! fn play_any_format<P: ChiptunePlayer>(player: &mut P) {
//!     println!("Playing: {}", player.metadata().title());
//!     player.play();
//!
//!     let mut buffer = vec![0.0; 4096];
//!     while player.state() == PlaybackState::Playing {
//!         player.generate_samples_into(&mut buffer);
//!         // ... send buffer to audio device
//!     }
//! }
//! ```

#![warn(missing_docs)]

mod cached_player;
mod metadata;
mod player;
pub mod visualization;

pub use cached_player::{CacheablePlayer, CachedPlayer, DEFAULT_CACHE_SIZE, SampleCache};
pub use metadata::{BasicMetadata, MetadataFields, PlaybackMetadata};
pub use player::{ChiptunePlayer, ChiptunePlayerBase, PlaybackState};
pub use visualization::{
    MAX_CHANNEL_COUNT, MAX_PSG_COUNT, SPECTRUM_BINS, SPECTRUM_DECAY, SpectrumAnalyzer,
    WaveformSynthesizer, freq_to_bin,
};

// ============================================================================
// Common Constants
// ============================================================================

/// Standard audio sample rate (44.1 kHz CD quality).
pub const DEFAULT_SAMPLE_RATE: u32 = 44_100;

/// PAL frame rate (50 Hz) - used by Atari ST, Amiga, and most European systems.
pub const FRAME_RATE_PAL: u32 = 50;

/// NTSC frame rate (60 Hz) - used by some American systems.
pub const FRAME_RATE_NTSC: u32 = 60;

/// Standard Atari ST master clock frequency (2 MHz).
pub const ATARI_ST_CLOCK: u32 = 2_000_000;

/// Number of audio channels per YM2149 PSG chip.
pub const CHANNELS_PER_PSG: usize = 3;
