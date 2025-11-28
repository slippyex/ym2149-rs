//! YM2149 PSG Emulation Domain
//!
//! Core Yamaha YM2149 Programmable Sound Generator emulation for the Atari ST,
//! including tone generation, envelope control, noise synthesis, and audio mixing.
//!
//! Implementation:
//! - `chip` - Integer-accurate, hardware-accurate core implementation

// Internal modules
pub mod chip;
pub mod constants;
mod empiric_dac;
pub mod psg_bank;

// Re-export public API
pub use chip::Ym2149;
pub use constants::get_volume;
pub use psg_bank::PsgBank;
