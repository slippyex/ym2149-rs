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

/// YM attribute flags shared across YM5/YM6 formats
pub(crate) const ATTR_STREAM_INTERLEAVED: u32 = 1;
pub(crate) const ATTR_DRUM_4BIT: u32 = 4;
pub(crate) const ATTR_LOOP_MODE: u32 = 16;

/// Lookup table for expanding 4-bit DigiDrum samples (matches ST-Sound reference)
const DIGIDRUM_4BIT_TABLE: [u8; 16] = [0, 1, 2, 2, 4, 6, 9, 12, 17, 24, 35, 48, 72, 103, 165, 255];

/// Expand 4-bit DigiDrum samples into 8-bit amplitude values
pub(crate) fn decode_4bit_digidrum(data: &[u8]) -> Vec<u8> {
    data.iter()
        .map(|byte| DIGIDRUM_4BIT_TABLE[(byte & 0x0F) as usize])
        .collect()
}
