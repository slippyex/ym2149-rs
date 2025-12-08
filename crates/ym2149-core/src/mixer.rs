//! Audio mixer and output stage
//!
//! The mixer combines tone and noise signals according to register R7,
//! applies volume/envelope levels, and handles special effects like DigiDrum.

use crate::generators::NUM_CHANNELS;
use crate::tables::{MASKS, YM2149_LOG_LEVELS};

/// Maximum output level for normalization
pub const MAX_LEVEL: u32 = 10922;

/// Mixer configuration from register R7
#[derive(Clone, Debug, Default)]
pub struct MixerConfig {
    /// Tone enable mask (inverted in hardware: 0 = enabled)
    tone_mask: u32,
    /// Noise enable mask (inverted in hardware: 0 = enabled)
    noise_mask: u32,
}

impl MixerConfig {
    /// Update mixer config from register R7 value
    #[inline]
    pub fn set_from_register(&mut self, value: u8) {
        self.tone_mask = MASKS[(value & 0x07) as usize];
        self.noise_mask = MASKS[((value >> 3) & 0x07) as usize];
    }

    /// Get the combined gate mask for all channels
    #[inline]
    pub fn compute_gate_mask(&self, tone_edges: u32, noise_mask: u32) -> u32 {
        (tone_edges | self.tone_mask) & (noise_mask | self.noise_mask)
    }
}

/// Channel state for mixing
#[derive(Clone, Debug, Default)]
pub struct ChannelState {
    /// User mute flag
    pub muted: bool,
    /// DigiDrum sample override
    pub drum_override: Option<f32>,
    /// Last computed output level (0.0 to 1.0)
    pub last_output: f32,
}

/// Audio mixer and output stage
#[derive(Clone, Debug, Default)]
pub struct Mixer {
    /// Mixer configuration
    pub config: MixerConfig,
    /// Per-channel state
    pub channels: [ChannelState; NUM_CHANNELS],
}

impl Mixer {
    /// Create a new mixer
    pub fn new() -> Self {
        Self::default()
    }

    /// Compute channel levels from volume registers and envelope
    ///
    /// # Arguments
    ///
    /// * `volume_regs` - Volume registers R8, R9, R10
    /// * `envelope_level` - Current envelope level (0-31)
    /// * `gate_mask` - Combined tone/noise gate mask
    ///
    /// # Returns
    ///
    /// Packed level values (5 bits per channel)
    #[inline]
    pub fn compute_levels(&self, volume_regs: [u8; 3], envelope_level: u32, gate_mask: u32) -> u32 {
        let mut levels: u32 = 0;

        for (i, &vol_reg) in volume_regs.iter().enumerate() {
            let level = if (vol_reg & 0x10) != 0 {
                // Envelope mode
                envelope_level
            } else {
                // Fixed volume (shift left to match envelope range)
                (vol_reg as u32) << 1
            };
            levels |= level << (i * 5);
        }

        // Apply gate mask
        levels & gate_mask
    }

    /// Compute final output for a channel
    ///
    /// # Arguments
    ///
    /// * `channel` - Channel index (0-2)
    /// * `level_index` - Level from compute_levels (0-31)
    /// * `half_amplitude` - Whether to halve amplitude (period <= 1)
    ///
    /// # Returns
    ///
    /// Output level (0 to MAX_LEVEL)
    #[inline]
    pub fn compute_channel_output(
        &mut self,
        channel: usize,
        level_index: u32,
        half_amplitude: bool,
    ) -> u32 {
        let state = &mut self.channels[channel];

        let output = if state.muted {
            0
        } else if let Some(drum_sample) = state.drum_override {
            // DigiDrum: scale 0-255 sample to YM volume range
            ((drum_sample * 255.0 / 6.0) as u32).min(MAX_LEVEL)
        } else {
            let base_level = YM2149_LOG_LEVELS[level_index as usize];
            if half_amplitude {
                base_level >> 1
            } else {
                base_level
            }
        };

        state.last_output = output as f32 / MAX_LEVEL as f32;
        output
    }

    /// Get the last output levels for all channels
    #[inline]
    pub fn channel_outputs(&self) -> (f32, f32, f32) {
        (
            self.channels[0].last_output,
            self.channels[1].last_output,
            self.channels[2].last_output,
        )
    }

    /// Set mute state for a channel
    #[inline]
    pub fn set_mute(&mut self, channel: usize, muted: bool) {
        if channel < NUM_CHANNELS {
            self.channels[channel].muted = muted;
        }
    }

    /// Check if channel is muted
    #[inline]
    pub fn is_muted(&self, channel: usize) -> bool {
        self.channels.get(channel).is_some_and(|c| c.muted)
    }

    /// Set drum sample override for a channel
    #[inline]
    pub fn set_drum_override(&mut self, channel: usize, sample: Option<f32>) {
        if channel < NUM_CHANNELS {
            self.channels[channel].drum_override = sample;
        }
    }

    /// Reset mixer state
    pub fn reset(&mut self) {
        self.config = MixerConfig::default();
        for channel in &mut self.channels {
            channel.drum_override = None;
            channel.last_output = 0.0;
            // Note: mute state preserved
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mixer_config() {
        let mut config = MixerConfig::default();

        // R7 = 0x38 means tone enabled on all, noise disabled
        config.set_from_register(0x38);
        assert_eq!(config.tone_mask, 0); // 0 = enabled

        // R7 = 0x3F means everything disabled
        config.set_from_register(0x3F);
        assert_eq!(config.tone_mask, MASKS[7]);
    }

    #[test]
    fn test_channel_mute() {
        let mut mixer = Mixer::new();

        assert!(!mixer.is_muted(0));
        mixer.set_mute(0, true);
        assert!(mixer.is_muted(0));
        assert!(!mixer.is_muted(1));
    }

    #[test]
    fn test_drum_override() {
        let mut mixer = Mixer::new();

        mixer.set_drum_override(0, Some(128.0));
        let output = mixer.compute_channel_output(0, 0, false);

        // Should use drum sample, not normal level
        assert!(output > 0);

        mixer.set_drum_override(0, None);
        let output_normal = mixer.compute_channel_output(0, 0, false);

        // With level 0 and no drum, should be minimal
        assert!(output_normal < output);
    }
}
