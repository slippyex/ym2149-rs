//! GIST sound driver module.
//!
//! Contains the core driver, sound definition, and voice state types for
//! playing GIST sound effects on a YM2149 PSG chip.

pub mod driver;
pub mod gist_sound;
pub(crate) mod voice;

/// GIST sound driver tick rate in Hz.
/// The driver processes all voices at this rate.
///
/// Design rationale:
/// - The original GIST driver on the Atari ST uses Timer C at 200 Hz
/// - This rate provides smooth envelope transitions at 5ms resolution
/// - Balance between CPU usage and audio quality for retro sound effects
pub const TICK_RATE: u32 = 200;
