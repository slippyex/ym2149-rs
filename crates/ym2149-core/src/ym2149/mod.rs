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
pub mod envelope;
pub mod mixer;
pub mod psg_bank;
pub mod registers;
pub mod tiny;

// Re-export public API
pub use chip::Ym2149;
pub use constants::get_volume;
pub use psg_bank::PsgBank;
pub use registers::RegisterBank;
pub use tiny::TinyYm2149;
