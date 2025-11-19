//! Format profile abstraction describing how to interpret YM frames.
//!
//! Each format (YM2, YM5, YM6, etc.) has slightly different rules for how
//! register frames should be interpreted and which embedded effects exist.

use crate::parser::effects::{EffectCommand, Ym6EffectDecoder, decode_effects_ym5};

/// High-level playback behavior for a parsed song.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatMode {
    /// Legacy formats without special handling (YM3/YM4).
    Basic,
    /// YM2 (Mad Max) frames with digi-drum/mixer quirks.
    Ym2,
    /// YM5 frames with inline effect encoding.
    Ym5,
    /// YM6 frames with extended effect encoding.
    Ym6,
}

/// Trait implemented by concrete format adapters.
pub trait FormatProfile: Send + Sync {
    /// Return the logical format mode.
    fn mode(&self) -> FormatMode;

    /// Allow format to mutate register frame before it is written.
    fn preprocess_frame(&mut self, _regs: &mut [u8; 16]) {}

    /// Decode effect commands encoded within the current frame.
    fn decode_effects(&mut self, _regs: &[u8; 16]) -> Vec<EffectCommand> {
        Vec::new()
    }
}

/// Create a format profile for the given mode.
pub fn create_profile(mode: FormatMode) -> Box<dyn FormatProfile> {
    match mode {
        FormatMode::Basic => Box::new(BasicProfile),
        FormatMode::Ym2 => Box::new(Ym2Profile),
        FormatMode::Ym5 => Box::new(Ym5Profile),
        FormatMode::Ym6 => Box::new(Ym6Profile::default()),
    }
}

struct BasicProfile;

impl FormatProfile for BasicProfile {
    fn mode(&self) -> FormatMode {
        FormatMode::Basic
    }
}

struct Ym2Profile;

impl FormatProfile for Ym2Profile {
    fn mode(&self) -> FormatMode {
        FormatMode::Ym2
    }
}

struct Ym5Profile;

impl FormatProfile for Ym5Profile {
    fn mode(&self) -> FormatMode {
        FormatMode::Ym5
    }

    fn decode_effects(&mut self, regs: &[u8; 16]) -> Vec<EffectCommand> {
        decode_effects_ym5(regs)
    }
}

#[derive(Default)]
struct Ym6Profile {
    decoder: Ym6EffectDecoder,
}

impl FormatProfile for Ym6Profile {
    fn mode(&self) -> FormatMode {
        FormatMode::Ym6
    }

    fn decode_effects(&mut self, regs: &[u8; 16]) -> Vec<EffectCommand> {
        self.decoder.decode_effects(regs).to_vec()
    }
}
