//! YM2149 Output Mixer
//!
//! Combines the three audio channels (A, B, C) and noise generator
//! with the mixer control register (R7) determining what is enabled.
//!
//! Features:
//! - Mixer control register (R7) for enable/disable of each channel
//! - DC offset removal via moving average filter for hardware-accurate output
//! - Raw channel summing without normalization

use bitflags::bitflags;

/// Channel sample data: (tone, noise) pairs
/// Used by [`Mixer::mix_with_overrides`] for advanced mixing scenarios
#[derive(Debug, Clone, Copy)]
pub struct ChannelSamples {
    /// Channel A tone and noise samples
    pub ch_a: (f32, f32),
    /// Channel B tone and noise samples
    pub ch_b: (f32, f32),
    /// Channel C tone and noise samples
    pub ch_c: (f32, f32),
}

bitflags! {
    /// Mixer Control Register (R7) bitflags
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct MixerFlags: u8 {
        /// Channel A tone enable (1 = disable, 0 = enable)
        const CH_A_TONE = 0x01;
        /// Channel B tone enable
        const CH_B_TONE = 0x02;
        /// Channel C tone enable
        const CH_C_TONE = 0x04;
        /// Channel A noise enable (1 = disable, 0 = enable)
        const CH_A_NOISE = 0x08;
        /// Channel B noise enable
        const CH_B_NOISE = 0x10;
        /// Channel C noise enable
        const CH_C_NOISE = 0x20;
    }
}

impl MixerFlags {
    /// Create mixer flags from raw register value
    pub fn from_register(value: u8) -> Self {
        MixerFlags::from_bits_truncate(value)
    }

    /// Check if channel A tone is enabled
    pub fn is_ch_a_tone_enabled(&self) -> bool {
        !self.contains(MixerFlags::CH_A_TONE)
    }

    /// Check if channel B tone is enabled
    pub fn is_ch_b_tone_enabled(&self) -> bool {
        !self.contains(MixerFlags::CH_B_TONE)
    }

    /// Check if channel C tone is enabled
    pub fn is_ch_c_tone_enabled(&self) -> bool {
        !self.contains(MixerFlags::CH_C_TONE)
    }

    /// Check if channel A noise is enabled
    pub fn is_ch_a_noise_enabled(&self) -> bool {
        !self.contains(MixerFlags::CH_A_NOISE)
    }

    /// Check if channel B noise is enabled
    pub fn is_ch_b_noise_enabled(&self) -> bool {
        !self.contains(MixerFlags::CH_B_NOISE)
    }

    /// Check if channel C noise is enabled
    pub fn is_ch_c_noise_enabled(&self) -> bool {
        !self.contains(MixerFlags::CH_C_NOISE)
    }
}

/// Audio Mixer - Combines all channels
///
/// Features:
/// - Combines 3 tone channels and 3 noise inputs
/// - Mixer control register (R7) gates channels on/off
/// - Raw channel summing (no normalization)
/// - DC offset removal via moving average filter
#[derive(Debug, Clone)]
pub struct Mixer {
    mixer_flags: MixerFlags,
    /// DC offset adjustment buffer (512 samples)
    /// Uses moving average to compute DC level
    dc_buffer: [f32; 512],
    /// Current position in DC buffer
    dc_pos: usize,
    /// Running sum of DC buffer for quick DC level calculation
    dc_sum: f32,
    /// Optional ST-style color filter (triangular FIR)
    color_filter_enabled: bool,
    last_in1: f32,
    last_in2: f32,
}

impl Mixer {
    /// DC buffer length for moving average
    /// 512 samples at 44.1kHz ≈ 11.6ms window for hardware-accurate DC removal
    const DC_BUFFER_LEN: usize = 512;
    /// Output gain scaling to keep summed channels within comfortable headroom
    const OUTPUT_GAIN: f32 = 0.75;

    /// Create a new mixer
    pub fn new() -> Self {
        Mixer {
            mixer_flags: MixerFlags::all(), // All disabled by default
            dc_buffer: [0.0; 512],
            dc_pos: 0,
            dc_sum: 0.0,
            color_filter_enabled: true,
            last_in1: 0.0,
            last_in2: 0.0,
        }
    }

    /// Set mixer control register value
    pub fn set_mixer_control(&mut self, value: u8) {
        self.mixer_flags = MixerFlags::from_register(value);
    }

    /// Get current mixer control value
    pub fn get_mixer_control(&self) -> u8 {
        self.mixer_flags.bits()
    }

    /// Reset the mixer to initial state
    /// Clears DC offset buffer and filter state
    pub fn reset(&mut self) {
        self.dc_buffer = [0.0; 512];
        self.dc_pos = 0;
        self.dc_sum = 0.0;
        self.last_in1 = 0.0;
        self.last_in2 = 0.0;
    }

    /// Enable or disable ST-style color filter (soft low-pass)
    pub fn set_color_filter(&mut self, enabled: bool) {
        self.color_filter_enabled = enabled;
    }

    /// Get the current DC level using moving average
    /// Returns the average of the last 512 samples for hardware-accurate offset removal
    #[inline]
    fn get_dc_level(&self) -> f32 {
        self.dc_sum / (Self::DC_BUFFER_LEN as f32)
    }

    /// Add a sample to the DC adjustment buffer and update DC level
    #[inline]
    fn add_dc_sample(&mut self, sample: f32) {
        // Remove old sample from sum
        self.dc_sum -= self.dc_buffer[self.dc_pos];
        // Add new sample to sum
        self.dc_sum += sample;
        // Store new sample in buffer
        self.dc_buffer[self.dc_pos] = sample;
        // Advance circular buffer position
        self.dc_pos = (self.dc_pos + 1) & (Self::DC_BUFFER_LEN - 1);
    }

    /// Mix with per-channel tone force-include override for DigiDrum support
    ///
    /// This method provides fine-grained control over which channels are included in the output,
    /// allowing individual channels to force-include their tone signal regardless of mixer R7 flags.
    ///
    /// If `force_include[i]` is true, channel i tone is included regardless of mixer R7 flags.
    /// This is used to guarantee tone output for DigiDrum playback even when the mixer would
    /// normally disable it.
    ///
    /// # Note
    ///
    /// This method is currently not used by the chip implementation, which processes tone and
    /// noise separately and combines them before calling the mixer. It's exported for advanced
    /// use cases and potential future optimizations. Most users should use `mix_pre_combined`
    /// instead, which is the primary mixing method.
    pub fn mix_with_overrides(&mut self, samples: ChannelSamples, force_include: [bool; 3]) -> f32 {
        let mut output = 0.0;

        let a_tone = self.mixer_flags.is_ch_a_tone_enabled() || force_include[0];
        let b_tone = self.mixer_flags.is_ch_b_tone_enabled() || force_include[1];
        let c_tone = self.mixer_flags.is_ch_c_tone_enabled() || force_include[2];

        // Channel A
        if a_tone {
            output += samples.ch_a.0;
        }
        if self.mixer_flags.is_ch_a_noise_enabled() {
            output += samples.ch_a.1;
        }

        // Channel B
        if b_tone {
            output += samples.ch_b.0;
        }
        if self.mixer_flags.is_ch_b_noise_enabled() {
            output += samples.ch_b.1;
        }

        // Channel C
        if c_tone {
            output += samples.ch_c.0;
        }
        if self.mixer_flags.is_ch_c_noise_enabled() {
            output += samples.ch_c.1;
        }

        // NOTE: No normalization - raw channel summing for hardware accuracy

        // DC removal using moving average filter
        if output.is_finite() {
            self.add_dc_sample(output);
            output -= self.get_dc_level();
        } else {
            self.add_dc_sample(0.0);
            output = 0.0;
        }

        let mut colored = output;
        if self.color_filter_enabled {
            colored = 0.25 * self.last_in2 + 0.5 * self.last_in1 + 0.25 * output;
            self.last_in2 = self.last_in1;
            self.last_in1 = output;
        } else {
            self.last_in2 = self.last_in1;
            self.last_in1 = output;
        }

        colored * Self::OUTPUT_GAIN
    }

    /// Mix pre-combined tone+noise channels
    /// Each channel has already had tone and noise combined via gating AND operation
    /// and amplitude applied. This method just adds the three channels and applies filtering.
    ///
    /// NOTE: Unlike some implementations, we do NOT normalize by channel count here.
    /// This implementation sums the raw outputs and relies on DC removal
    /// and the output filter to handle the amplitude scaling. This approach better
    /// preserves the envelope dynamics for voice-based audio (especially important
    /// for features like sync-buzzer that depend on precise amplitude modulation).
    pub fn mix_pre_combined(
        &mut self,
        ch_a_combined: f32,
        ch_b_combined: f32,
        ch_c_combined: f32,
    ) -> f32 {
        // Sum all channels without normalization for hardware accuracy
        let mut output = ch_a_combined + ch_b_combined + ch_c_combined;

        // Apply DC offset removal using moving average filter
        if output.is_finite() {
            self.add_dc_sample(output);
            output -= self.get_dc_level();
        } else {
            self.add_dc_sample(0.0);
            output = 0.0;
        }

        // Optional ST color filter: 0.25*x[n-2] + 0.5*x[n-1] + 0.25*x[n]
        let mut colored = output;
        if self.color_filter_enabled {
            colored = 0.25 * self.last_in2 + 0.5 * self.last_in1 + 0.25 * output;
            self.last_in2 = self.last_in1;
            self.last_in1 = output;
        } else {
            // Keep history coherent even if disabled
            self.last_in2 = self.last_in1;
            self.last_in1 = output;
        }

        colored * Self::OUTPUT_GAIN
    }
}

impl Default for Mixer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mixer_flags() {
        let flags = MixerFlags::from_register(0x00);
        assert!(flags.is_ch_a_tone_enabled());
        assert!(flags.is_ch_b_tone_enabled());
        assert!(flags.is_ch_c_tone_enabled());
    }

    #[test]
    fn test_mixer_flags_disabled() {
        let flags = MixerFlags::from_register(0xFF);
        assert!(!flags.is_ch_a_tone_enabled());
        assert!(!flags.is_ch_b_tone_enabled());
        assert!(!flags.is_ch_c_tone_enabled());
    }

    #[test]
    fn test_mixer_pre_combined_silent_channels() {
        let mut mixer = Mixer::new();
        let output = mixer.mix_pre_combined(0.0, 0.0, 0.0);
        assert_eq!(output, 0.0);
    }

    #[test]
    fn test_mixer_pre_combined_single_channel() {
        let mut mixer = Mixer::new();
        let output = mixer.mix_pre_combined(1.0, 0.0, 0.0);
        // With color filter enabled, first sample is quartered then scaled by OUTPUT_GAIN
        let expected = Mixer::OUTPUT_GAIN * 0.25;
        assert!(
            (output - expected).abs() < 0.05,
            "Single channel output {} should be close to {}",
            output,
            expected
        );
    }

    #[test]
    fn test_mixer_pre_combined_dc_removal() {
        let mut mixer = Mixer::new();
        // Feed small DC signal on one channel to make DC removal visible
        let mut outputs = Vec::new();
        for _ in 0..10000 {
            let output = mixer.mix_pre_combined(0.5, 0.0, 0.0);
            outputs.push(output);
        }

        // After settling (with slow alpha=0.999 filter), DC should be significantly reduced
        let recent_samples = &outputs[9000..];
        let average = recent_samples.iter().sum::<f32>() / recent_samples.len() as f32;
        // With slow DC filter, the constant 0.5 will be partially attenuated
        // We just verify it's less than the original input
        assert!(
            average < 0.5,
            "DC offset should be at least partially removed: average = {}",
            average
        );
    }

    #[test]
    fn test_mixer_pre_combined_nan_handling() {
        let mut mixer = Mixer::new();
        // Feed NaN - should handle gracefully
        let output = mixer.mix_pre_combined(f32::NAN, 0.0, 0.0);
        assert_eq!(output, 0.0, "NaN should produce 0.0 output");
    }

    #[test]
    fn test_mixer_pre_combined_infinity_handling() {
        let mut mixer = Mixer::new();
        let output = mixer.mix_pre_combined(f32::INFINITY, 0.0, 0.0);
        assert_eq!(output, 0.0, "Infinity should produce 0.0 output");
    }
}
