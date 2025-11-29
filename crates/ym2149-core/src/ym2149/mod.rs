//! YM2149 PSG Emulation Domain
//!
//! Core Yamaha YM2149 Programmable Sound Generator emulation for the Atari ST,
//! including tone generation, envelope control, noise synthesis, and audio mixing.
//!
//! Implementation based on Leonard/Oxygene's AtariAudio - a tiny & cycle accurate
//! ym2149 emulation that operates at original YM freq divided by 8 (250Khz).

// Internal modules
pub mod chip;
pub mod constants;
pub mod psg_bank;
mod tables;

// Re-export public API
pub use chip::Ym2149;
pub use constants::get_volume;
pub use psg_bank::PsgBank;
