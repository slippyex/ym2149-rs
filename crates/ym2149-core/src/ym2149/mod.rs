//! YM2149 PSG Emulation Domain
//!
//! Core Yamaha YM2149 Programmable Sound Generator emulation for the Atari ST,
//! including tone generation, envelope control, noise synthesis, and audio mixing.
//!
//! Cycle accurate YM2149 emulation that operates at original YM freq divided by 8 (250kHz).
//!
//! # Architecture
//!
//! The emulator is organized into focused components:
//!
//! - [`generators`] - Tone, noise, and envelope generators
//! - [`mixer`] - Audio mixing and output stage
//! - [`dc_filter`] - DC offset removal
//! - [`tables`] - Hardware lookup tables (volumes, envelopes)

// Internal modules
pub mod chip;
pub mod constants;
mod dc_filter;
mod generators;
mod mixer;
pub mod psg_bank;
mod tables;

// Re-export public API
pub use chip::Ym2149;
pub use constants::get_volume;
pub use psg_bank::PsgBank;
