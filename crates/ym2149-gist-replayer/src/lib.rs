//! GIST Sound File Parser and Multi-PSG Player for YM2149
//!
//! This crate provides a parser and player for GIST (Graphics, Images, Sound, Text)
//! sound effect files, originally used on the Atari ST. GIST was developed by Dave Becker
//! and distributed by Antic Software in the late 1980s.
//!
//! # Overview
//!
//! GIST sound effects are 112-byte definitions that describe complex synthesizer patches
//! with ADSR-style envelopes for volume, frequency, and noise, plus LFO modulation for
//! vibrato, tremolo, and noise effects.
//!
//! The driver processes sounds at 200 Hz (matching the Atari ST Timer C rate) and supports
//! up to 3 simultaneous voices on a single YM2149 PSG chip.
//!
//! # Quick Start
//!
//! For simple playback, use `GistPlayer`:
//!
//! ```rust,no_run
//! use ym2149_gist_replayer::{GistPlayer, GistSound};
//!
//! // Load and play a sound effect
//! let sound = GistSound::load("effect.snd").unwrap();
//! let mut player = GistPlayer::new();
//!
//! player.play_sound(&sound, None, None);
//!
//! // Generate audio samples
//! while player.is_playing() {
//!     let samples = player.generate_samples(882); // ~20ms at 44100 Hz
//!     // Send samples to audio output...
//! }
//! ```
//!
//! # Low-Level API
//!
//! For more control, use `GistDriver` directly with a `Ym2149` chip:
//!
//! ```rust,no_run
//! use ym2149::Ym2149;
//! use ym2149_gist_replayer::{GistDriver, GistSound, TICK_RATE};
//!
//! let sound = GistSound::load("effect.snd").unwrap();
//! let mut chip = Ym2149::new();
//! let mut driver = GistDriver::new();
//!
//! driver.snd_on(&mut chip, &sound, None, None, -1, i16::MAX - 1);
//!
//! // In your audio loop, call tick() at 200 Hz
//! while driver.is_playing() {
//!     driver.tick(&mut chip);
//!     // Generate samples from chip...
//! }
//! ```
//!
//! # Sound Structure
//!
//! Each GIST sound contains:
//! - **Duration**: How long the sound plays in ticks (200 Hz)
//! - **Tone parameters**: Initial frequency, envelope, and LFO settings
//! - **Noise parameters**: Initial noise frequency, envelope, and LFO settings
//! - **Volume envelope**: ADSR-style envelope with LFO modulation
//!
//! Values use 16.16 fixed-point format for smooth envelope transitions.

mod gist;
mod player;

// Core types
pub use gist::TICK_RATE;
pub use gist::driver::GistDriver;
pub use gist::gist_sound::GistSound;

// High-level player
pub use player::{DEFAULT_SAMPLE_RATE, GistMetadata, GistPlayer};

// Re-export common traits for convenience
pub use ym2149_common::{ChiptunePlayer, ChiptunePlayerBase, PlaybackState};
