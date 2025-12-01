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
//! use ym2149_common::{ChiptunePlayer, PlaybackMetadata};
//!
//! fn play_any_format(player: &mut dyn ChiptunePlayer) {
//!     println!("Playing: {}", player.metadata().title());
//!     player.play();
//!
//!     let mut buffer = vec![0.0; 4096];
//!     player.generate_samples_into(&mut buffer);
//! }
//! ```

#![warn(missing_docs)]

mod metadata;
mod player;

pub use metadata::{BasicMetadata, MetadataFields, PlaybackMetadata};
pub use player::{ChiptunePlayer, PlaybackState};
