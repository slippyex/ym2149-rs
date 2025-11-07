//! File Format Support
//!
//! Parser implementations for various YM2149 music file formats:
//! - YM6 format (Atari ST chiptune register dumps)
//! - YM format (legacy format)
//! - Raw register dumps
//! - Effects decoding (YM5/YM6 special effects)

pub mod effects;
pub mod raw;
pub mod ym;
pub mod ym6;

pub use effects::{EffectCommand, MFP_CLOCK, Ym6EffectDecoder, decode_effects_ym5};
pub use raw::RawParser;
pub use ym::{YmMetadata, YmParser};
pub use ym6::Ym6Parser;

use crate::Result;

/// Trait for parsing music file formats into register frame sequences
pub trait FormatParser {
    /// Parse file data and return register frames
    fn parse(&self, data: &[u8]) -> Result<Vec<[u8; 16]>>;

    /// Get parser name
    fn name(&self) -> &str;
}
