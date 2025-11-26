//! Arkos Tracker 3 (AKS) file parser and multi-PSG player
//!
//! This crate provides support for loading and playing Arkos Tracker 3 songs (.aks format),
//! which is an XML-based tracker format supporting multiple PSG chips for expanded polyphony.
//!
//! # Features
//!
//! - Load AKS (Arkos Tracker 3) XML files
//! - Multi-PSG support (n chips = n×3 channels)
//! - Per-PSG frequency configuration (CPC, Atari ST, PlayCity, etc.)
//! - Instruments with software/hardware envelopes
//! - Arpeggios and pitch tables
//! - Pattern-based sequencing with positions
//! - Subsong support
//!
//! # Quick Start
//!
//! ```no_run
//! use ym2149_arkos_replayer::{load_aks, ArkosPlayer};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let data = std::fs::read("song.aks")?;
//! let song = load_aks(&data)?;
//!
//! println!("Title: {}", song.metadata.title);
//! println!("Subsongs: {}", song.subsongs.len());
//! if !song.subsongs.is_empty() {
//!     println!("PSGs: {}", song.subsongs[0].psgs.len());
//!
//!     let mut player = ArkosPlayer::new(song, 0)?; // Subsong 0
//!     player.play()?;
//!
//!     let samples = player.generate_samples(882);
//! }
//! # Ok(())
//! # }
//! ```

#![warn(missing_docs)]

pub mod channel_player;
pub mod effect_context;
pub mod effects;
pub mod error;
pub mod expression;
pub mod fixed_point;
pub mod format;
pub mod parser;
pub mod player;
pub mod psg;
pub mod psg_registers;
pub mod psg_registers_converter;

// Re-export public API
pub use error::{ArkosError, Result};
pub use format::*;
pub use parser::load_aks;
pub use player::{ArkosMetadata, ArkosPlayer};

// Re-export parser types for advanced usage
pub use parser::{FormatVersion, ParseState};

// Re-export unified player trait from ym2149-common
pub use ym2149_common::{ChiptunePlayer, PlaybackMetadata};
