//! Shared visualization utilities for YM2149 oscilloscope and spectrum display.
//!
//! This module provides register-based visualization that works across all frontends
//! (Bevy, CLI TUI) and all formats (YM, AKS, AY, SNDH). Unlike FFT-based analysis,
//! this approach synthesizes waveforms directly from register state, ensuring
//! visualization works even when digidrums or STE-DAC bypass the PSG.
//!
//! # Example
//!
//! ```ignore
//! use ym2149_common::visualization::{WaveformSynthesizer, SpectrumAnalyzer};
//! use ym2149_common::ChannelStates;
//!
//! let mut waveform = WaveformSynthesizer::new();
//! let mut spectrum = SpectrumAnalyzer::new();
//!
//! // Update from register state each frame
//! let channel_states = ChannelStates::from_registers(&registers);
//! waveform.update(&channel_states);
//! spectrum.update(&channel_states);
//!
//! // Get data for rendering
//! let samples = waveform.get_samples();
//! let bins = spectrum.get_bins();
//! ```

use crate::channel_state::ChannelStates;
use std::collections::VecDeque;

// ============================================================================
// Constants
// ============================================================================

/// Maximum number of PSG chips supported.
pub const MAX_PSG_COUNT: usize = 4;

/// Maximum number of channels (4 PSGs × 3 channels).
pub const MAX_CHANNEL_COUNT: usize = MAX_PSG_COUNT * 3;

/// Number of samples to keep for waveform display.
pub const WAVEFORM_SIZE: usize = 256;

/// Sample rate for waveform generation (visual only, not audio).
/// Uses 44.1kHz as a good balance between visual smoothness and performance.
pub const VISUAL_SAMPLE_RATE: f32 = 44100.0;

/// Number of samples to generate per frame update (~20ms at 50Hz).
pub const SAMPLES_PER_UPDATE: usize = 64;

/// Number of octaves covered by spectrum display.
/// 8 octaves covers C1 (~33 Hz) to B8 (~8 kHz), full YM2149 range.
pub const SPECTRUM_OCTAVES: usize = 8;

/// Bins per octave (4 = minor-third resolution for compact display).
pub const BINS_PER_OCTAVE: usize = 4;

/// Number of spectrum bins (8 octaves × 4 bins per octave = 32 bins).
pub const SPECTRUM_BINS: usize = SPECTRUM_OCTAVES * BINS_PER_OCTAVE;

/// Decay factor for spectrum bars (0.85 = fast release, responsive visualization).
pub const SPECTRUM_DECAY: f32 = 0.85;

/// Base frequency for spectrum bins: C1 = 32.703 Hz (MIDI note 24).
pub const SPECTRUM_BASE_FREQ: f32 = 32.703;

// ============================================================================
// Waveform Synthesizer
// ============================================================================

/// Synthesizes oscilloscope waveforms from YM2149 register state.
///
/// This generates per-channel waveforms based on the current register values,
/// producing square waves for tone, pseudo-random noise, and envelope-accurate
/// waveforms for buzz instruments.
///
/// Supports up to 4 PSGs (12 channels total) for multi-PSG configurations.
#[derive(Clone)]
pub struct WaveformSynthesizer {
    /// Ring buffer for waveform samples (per channel, up to 12).
    waveform: [VecDeque<f32>; MAX_CHANNEL_COUNT],
    /// Phase accumulators for waveform generation (per channel).
    phase: [f32; MAX_CHANNEL_COUNT],
    /// Number of active PSGs.
    psg_count: usize,
}

impl Default for WaveformSynthesizer {
    fn default() -> Self {
        Self::new()
    }
}

impl WaveformSynthesizer {
    /// Create a new waveform synthesizer.
    pub fn new() -> Self {
        Self {
            waveform: std::array::from_fn(|_| VecDeque::with_capacity(WAVEFORM_SIZE)),
            phase: [0.0; MAX_CHANNEL_COUNT],
            psg_count: 1,
        }
    }

    /// Set the number of active PSGs (1-4).
    pub fn set_psg_count(&mut self, count: usize) {
        self.psg_count = count.clamp(1, MAX_PSG_COUNT);
    }

    /// Get the number of active PSGs.
    pub fn psg_count(&self) -> usize {
        self.psg_count
    }

    /// Get the number of active channels.
    pub fn channel_count(&self) -> usize {
        self.psg_count * 3
    }

    /// Update waveforms from YM2149 channel states (single PSG, for backward compatibility).
    ///
    /// Call this once per frame to generate new waveform samples based on
    /// the current register state. This updates channels 0-2.
    pub fn update(&mut self, channel_states: &ChannelStates) {
        self.update_psg(0, channel_states);
    }

    /// Update waveforms for a specific PSG (0-3).
    ///
    /// Call this for each active PSG to update its 3 channels.
    pub fn update_psg(&mut self, psg_index: usize, channel_states: &ChannelStates) {
        if psg_index >= MAX_PSG_COUNT {
            return;
        }

        let base_channel = psg_index * 3;

        for (local_ch, ch_state) in channel_states.channels.iter().enumerate() {
            let global_ch = base_channel + local_ch;
            if global_ch >= MAX_CHANNEL_COUNT {
                break;
            }

            // Get frequency and amplitude for this channel
            let freq = ch_state.frequency_hz.unwrap_or(0.0);
            let has_output =
                ch_state.tone_enabled || ch_state.noise_enabled || ch_state.envelope_enabled;
            let has_amplitude = ch_state.amplitude > 0 || ch_state.envelope_enabled;

            let amplitude = if has_output && has_amplitude {
                if ch_state.envelope_enabled {
                    1.0
                } else {
                    ch_state.amplitude_normalized
                }
            } else {
                0.0
            };

            // Calculate phase increment
            let phase_increment = if freq > 0.0 {
                freq / VISUAL_SAMPLE_RATE
            } else {
                0.0
            };

            // Get envelope shape for accurate waveform synthesis
            let envelope_shape = channel_states.envelope.shape;

            for _ in 0..SAMPLES_PER_UPDATE {
                let sample =
                    self.synthesize_sample(ch_state, global_ch, amplitude, envelope_shape, freq);

                // Add to waveform buffer
                if self.waveform[global_ch].len() >= WAVEFORM_SIZE {
                    self.waveform[global_ch].pop_front();
                }
                self.waveform[global_ch].push_back(sample);

                // Advance phase with proper wrapping (handles phase_increment > 1.0)
                self.phase[global_ch] = (self.phase[global_ch] + phase_increment).fract();
            }
        }
    }

    /// Update waveforms from multiple PSG register banks.
    ///
    /// Call this once per frame with all PSG register states.
    pub fn update_multi_psg(&mut self, register_banks: &[[u8; 16]], psg_count: usize) {
        self.set_psg_count(psg_count);

        for (psg_idx, registers) in register_banks.iter().enumerate().take(psg_count) {
            let channel_states = ChannelStates::from_registers(registers);
            self.update_psg(psg_idx, &channel_states);
        }
    }

    /// Synthesize a single sample for a channel.
    ///
    /// Handles different YM2149 sound types:
    /// - Pure tone: square wave
    /// - Pure noise: LFSR-like noise
    /// - Buzz/Envelope: envelope waveform (sawtooth, triangle, etc.)
    /// - Sync-buzzer: envelope with tone frequency modulation
    #[inline]
    fn synthesize_sample(
        &self,
        ch_state: &crate::channel_state::ChannelState,
        ch: usize,
        amplitude: f32,
        envelope_shape: u8,
        freq: f32,
    ) -> f32 {
        let phase = self.phase[ch];

        // Priority: Envelope/Buzz sounds take precedence for visualization
        // because they have the most interesting waveform shape.
        // Sync-buzzer: envelope_enabled + tone_period > 0 (freq used for pitch)
        // Pure buzz: envelope_enabled + tone_period = 0 (envelope freq for pitch)
        if ch_state.envelope_enabled && freq > 0.0 {
            // Envelope/Buzz: synthesize based on actual shape register
            // This includes sync-buzzer (tone+envelope) and pure buzz (envelope only)
            self.synthesize_envelope_sample(envelope_shape, phase, amplitude)
        } else if ch_state.tone_enabled && freq > 0.0 {
            // Pure tone: square wave
            if phase < 0.5 { amplitude } else { -amplitude }
        } else if ch_state.noise_enabled {
            // Noise: pseudo-random values scaled by amplitude
            // Use LFSR-like behavior based on phase
            let noise = (phase * 12345.0).sin() * 2.0 - 1.0;
            noise * amplitude * 0.7
        } else {
            0.0
        }
    }

    /// Synthesize envelope waveform based on YM2149 envelope shape.
    ///
    /// YM2149 envelope shapes (register 13, bits 0-3):
    /// - 0x00-0x03: Decay (\\\___)
    /// - 0x04-0x07: Attack (/____)
    /// - 0x08: Sawtooth down (\\\\\\\\)
    /// - 0x09: Decay one-shot (\\\___)
    /// - 0x0A: Triangle (/\\/\\)
    /// - 0x0B: Decay + hold high (\\¯¯¯)
    /// - 0x0C: Sawtooth up (////)
    /// - 0x0D: Attack + hold high (/¯¯¯)
    /// - 0x0E: Triangle inverted (\\/\\/)
    /// - 0x0F: Attack one-shot (/____)
    #[inline]
    fn synthesize_envelope_sample(&self, shape: u8, phase: f32, amplitude: f32) -> f32 {
        let sample = match shape & 0x0F {
            // Decay shapes: start high, go low
            0x00..=0x03 | 0x09 => {
                // Single decay: high to low
                1.0 - phase * 2.0
            }
            // Attack shapes: start low, go high
            0x04..=0x07 | 0x0F => {
                // Single attack: low to high
                phase * 2.0 - 1.0
            }
            // Sawtooth down: continuous decay
            0x08 => {
                // Repeating sawtooth down
                1.0 - phase * 2.0
            }
            // Triangle: /\/\/\
            0x0A => {
                // Triangle wave
                if phase < 0.5 {
                    phase * 4.0 - 1.0 // Rising: -1 to 1
                } else {
                    3.0 - phase * 4.0 // Falling: 1 to -1
                }
            }
            // Decay + hold high
            0x0B => {
                // Decay then hold at max
                if phase < 0.5 { 1.0 - phase * 4.0 } else { 1.0 }
            }
            // Sawtooth up: continuous attack
            0x0C => {
                // Repeating sawtooth up
                phase * 2.0 - 1.0
            }
            // Attack + hold high
            0x0D => {
                // Attack then hold at max
                if phase < 0.5 { phase * 4.0 - 1.0 } else { 1.0 }
            }
            // Triangle inverted: \/\/\/
            0x0E => {
                // Inverted triangle wave
                if phase < 0.5 {
                    1.0 - phase * 4.0 // Falling: 1 to -1
                } else {
                    phase * 4.0 - 3.0 // Rising: -1 to 1
                }
            }
            _ => 0.0,
        };

        sample * amplitude
    }

    /// Get waveform samples as a Vec for display (first 3 channels only for backward compat).
    ///
    /// Returns samples in the format `[amplitude_a, amplitude_b, amplitude_c]` per sample.
    pub fn get_samples(&self) -> Vec<[f32; 3]> {
        let len = self.waveform[0]
            .len()
            .min(self.waveform[1].len())
            .min(self.waveform[2].len());

        (0..len)
            .map(|i| {
                [
                    self.waveform[0].get(i).copied().unwrap_or(0.0),
                    self.waveform[1].get(i).copied().unwrap_or(0.0),
                    self.waveform[2].get(i).copied().unwrap_or(0.0),
                ]
            })
            .collect()
    }

    /// Get waveform for a specific channel (0-11 for multi-PSG).
    pub fn channel_waveform(&self, channel: usize) -> &VecDeque<f32> {
        &self.waveform[channel.min(MAX_CHANNEL_COUNT - 1)]
    }
}

// ============================================================================
// Spectrum Analyzer
// ============================================================================

/// Map a frequency to a spectrum bin index (note-aligned, minor-third resolution).
///
/// Returns bin 0-31 based on position (3 semitones per bin):
/// - Bin 0: C1 (32.7 Hz)
/// - Bin 4: C2 (65.4 Hz)
/// - Bin 12: C4 (262 Hz, middle C)
/// - Bin 16: C5 (523 Hz)
/// - Bin 28: C8 (4186 Hz)
/// - Bin 31: A#8 (~7458 Hz)
#[inline]
pub fn freq_to_bin(freq: f32) -> usize {
    if freq <= 0.0 {
        return 0;
    }
    // Calculate semitones above C1
    // bin = log2(freq / C1) * 12
    let octaves_above_c1 = (freq / SPECTRUM_BASE_FREQ).log2();
    let bin = (octaves_above_c1 * BINS_PER_OCTAVE as f32).round() as i32;
    bin.clamp(0, (SPECTRUM_BINS - 1) as i32) as usize
}

/// Register-based spectrum analyzer.
///
/// Maps YM2149 channel frequencies to note-aligned spectrum bins,
/// showing the actual notes being played rather than FFT analysis.
///
/// Supports up to 4 PSGs (12 channels total) for multi-PSG configurations.
#[derive(Clone)]
pub struct SpectrumAnalyzer {
    /// Per-channel spectrum magnitudes (up to 12 channels).
    spectrum: [[f32; SPECTRUM_BINS]; MAX_CHANNEL_COUNT],
    /// Combined spectrum (max across all active channels).
    combined: [f32; SPECTRUM_BINS],
    /// Number of active PSGs.
    psg_count: usize,
}

impl Default for SpectrumAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl SpectrumAnalyzer {
    /// Create a new spectrum analyzer.
    pub fn new() -> Self {
        Self {
            spectrum: [[0.0; SPECTRUM_BINS]; MAX_CHANNEL_COUNT],
            combined: [0.0; SPECTRUM_BINS],
            psg_count: 1,
        }
    }

    /// Set the number of active PSGs (1-4).
    pub fn set_psg_count(&mut self, count: usize) {
        self.psg_count = count.clamp(1, MAX_PSG_COUNT);
    }

    /// Get the number of active PSGs.
    pub fn psg_count(&self) -> usize {
        self.psg_count
    }

    /// Get the number of active channels.
    pub fn channel_count(&self) -> usize {
        self.psg_count * 3
    }

    /// Update spectrum from YM2149 channel states (single PSG, for backward compatibility).
    ///
    /// Call this once per frame. Applies decay to previous values
    /// and updates bins based on current frequencies. Updates channels 0-2.
    pub fn update(&mut self, channel_states: &ChannelStates) {
        self.update_psg(0, channel_states);
        self.update_combined();
    }

    /// Update spectrum for a specific PSG (0-3).
    ///
    /// Call this for each active PSG to update its 3 channels.
    pub fn update_psg(&mut self, psg_index: usize, channel_states: &ChannelStates) {
        if psg_index >= MAX_PSG_COUNT {
            return;
        }

        let base_channel = psg_index * 3;

        for (local_ch, ch_state) in channel_states.channels.iter().enumerate() {
            let global_ch = base_channel + local_ch;
            if global_ch >= MAX_CHANNEL_COUNT {
                break;
            }

            // Save previous value for decay
            let prev = self.spectrum[global_ch];

            // Reset current frame for this channel
            self.spectrum[global_ch] = [0.0; SPECTRUM_BINS];

            // Channel is active if it has amplitude AND some output enabled
            let has_output =
                ch_state.tone_enabled || ch_state.noise_enabled || ch_state.envelope_enabled;
            let has_amplitude = ch_state.amplitude > 0 || ch_state.envelope_enabled;
            let is_active = has_amplitude && has_output;

            if is_active {
                // For envelope mode, use full amplitude since envelope controls it dynamically
                let magnitude = if ch_state.envelope_enabled {
                    1.0
                } else {
                    ch_state.amplitude_normalized
                };

                // Handle tone frequency
                if ch_state.tone_enabled
                    && let Some(freq) = ch_state.frequency_hz
                    && freq > 0.0
                {
                    let bin = freq_to_bin(freq);
                    self.spectrum[global_ch][bin] = magnitude;
                }

                // Handle noise - spread across high frequency bins
                if ch_state.noise_enabled {
                    self.add_noise_to_spectrum(global_ch, channel_states.noise.period, magnitude);
                }

                // Handle envelope/buzz instruments (including sync-buzzer)
                if ch_state.envelope_enabled {
                    self.add_envelope_to_spectrum(global_ch, ch_state, channel_states, magnitude);
                }
            }

            // Apply decay to all bins
            for (bin, &prev_val) in prev.iter().enumerate() {
                if self.spectrum[global_ch][bin] < prev_val {
                    self.spectrum[global_ch][bin] = prev_val * SPECTRUM_DECAY;
                }
            }
        }
    }

    /// Update spectrum from multiple PSG register banks.
    ///
    /// Call this once per frame with all PSG register states.
    pub fn update_multi_psg(&mut self, register_banks: &[[u8; 16]], psg_count: usize) {
        self.set_psg_count(psg_count);

        for (psg_idx, registers) in register_banks.iter().enumerate().take(psg_count) {
            let channel_states = ChannelStates::from_registers(registers);
            self.update_psg(psg_idx, &channel_states);
        }

        self.update_combined();
    }

    /// Update the combined spectrum from all active channels.
    fn update_combined(&mut self) {
        let channel_count = self.channel_count();
        for (bin, combined) in self.combined.iter_mut().enumerate() {
            *combined = (0..channel_count)
                .map(|ch| self.spectrum[ch][bin])
                .fold(0.0, f32::max);
        }
    }

    /// Add noise contribution to spectrum.
    fn add_noise_to_spectrum(&mut self, ch: usize, noise_period: u8, magnitude: f32) {
        // Map noise period to bins: period 0 = high freq, period 31 = lower freq
        let noise_center = if noise_period == 0 {
            SPECTRUM_BINS - 2 // Very high frequency noise
        } else {
            let ratio = 1.0 - (noise_period as f32 / 31.0);
            ((ratio * 0.6 + 0.3) * (SPECTRUM_BINS - 1) as f32) as usize
        };

        // Spread noise across a few adjacent bins for "fuzzy" look
        let noise_mag = magnitude * 0.7;
        for offset in 0..=2 {
            let bin = (noise_center + offset).min(SPECTRUM_BINS - 1);
            self.spectrum[ch][bin] =
                self.spectrum[ch][bin].max(noise_mag * (1.0 - offset as f32 * 0.25));
        }
    }

    /// Add envelope/buzz contribution to spectrum.
    fn add_envelope_to_spectrum(
        &mut self,
        ch: usize,
        ch_state: &crate::channel_state::ChannelState,
        channel_states: &ChannelStates,
        magnitude: f32,
    ) {
        // Sync-buzzer: tone_period sets the pitch, envelope provides the timbre
        // For sync-buzzer: use tone frequency (even if tone is disabled in mixer)
        // For pure buzz: fall back to envelope frequency
        let buzz_freq = if ch_state.frequency_hz.is_some() && ch_state.tone_period > 0 {
            ch_state.frequency_hz
        } else {
            channel_states.envelope.frequency_hz
        };

        if let Some(freq) = buzz_freq
            && freq > 0.0
        {
            let bin = freq_to_bin(freq);
            self.spectrum[ch][bin] = self.spectrum[ch][bin].max(magnitude);
        }
    }

    /// Get combined spectrum bins (max across all channels).
    pub fn get_bins(&self) -> &[f32; SPECTRUM_BINS] {
        &self.combined
    }

    /// Get spectrum for a specific channel (0-11 for multi-PSG).
    pub fn channel_spectrum(&self, channel: usize) -> &[f32; SPECTRUM_BINS] {
        &self.spectrum[channel.min(MAX_CHANNEL_COUNT - 1)]
    }

    /// Get all per-channel spectrums (all 12 channels).
    pub fn all_channel_spectrums(&self) -> &[[f32; SPECTRUM_BINS]; MAX_CHANNEL_COUNT] {
        &self.spectrum
    }

    /// Compute high frequency ratio (bins 8-15 vs total).
    ///
    /// Useful for badges indicating "bright" or "treble" content.
    pub fn high_freq_ratio(&self, channel: usize) -> f32 {
        let ch = channel.min(2);
        let total_energy: f32 = self.spectrum[ch].iter().sum();
        let high_energy: f32 = self.spectrum[ch][8..].iter().sum();

        if total_energy > 0.01 {
            (high_energy / total_energy).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_freq_to_bin_c1() {
        // C1 = 32.703 Hz should map to bin 0
        assert_eq!(freq_to_bin(32.703), 0);
    }

    #[test]
    fn test_freq_to_bin_a4() {
        // A4 = 440 Hz is 3 octaves + 9 semitones above C1
        // With 4 bins per octave: bin = log2(440/32.703) * 4 ≈ 15
        // C1=0, C2=4, C3=8, C4=12, A4≈15
        let bin = freq_to_bin(440.0);
        assert!(
            bin >= 14 && bin <= 16,
            "A4 should be around bin 15, got {}",
            bin
        );
    }

    #[test]
    fn test_freq_to_bin_bounds() {
        assert_eq!(freq_to_bin(0.0), 0);
        assert_eq!(freq_to_bin(-100.0), 0);
        assert_eq!(freq_to_bin(20000.0), SPECTRUM_BINS - 1);
    }

    #[test]
    fn test_waveform_phase_wrapping() {
        let mut synth = WaveformSynthesizer::new();

        // Create channel states with very high frequency
        let mut regs = [0u8; 16];
        regs[0] = 1; // Very low period = very high frequency
        regs[7] = 0x3E; // Tone A enabled
        regs[8] = 0x0F; // Max amplitude

        let states = ChannelStates::from_registers(&regs);
        synth.update(&states);

        // Phase should always be in [0, 1)
        assert!(synth.phase[0] >= 0.0 && synth.phase[0] < 1.0);
    }

    #[test]
    fn test_spectrum_decay() {
        let mut analyzer = SpectrumAnalyzer::new();

        // Set up a tone on channel A
        let mut regs = [0u8; 16];
        regs[0] = 0x1C;
        regs[1] = 0x01; // Period 284 ≈ 440Hz
        regs[7] = 0x3E;
        regs[8] = 0x0F;

        let states = ChannelStates::from_registers(&regs);
        analyzer.update(&states);

        let initial_bin = freq_to_bin(440.0);
        let initial_value = analyzer.spectrum[0][initial_bin];
        assert!(initial_value > 0.0);

        // Now update with silence
        let silent_regs = [0u8; 16];
        let silent_states = ChannelStates::from_registers(&silent_regs);
        analyzer.update(&silent_states);

        // Value should have decayed but not be zero
        let decayed_value = analyzer.spectrum[0][initial_bin];
        assert!(decayed_value > 0.0);
        assert!(decayed_value < initial_value);
        assert!((decayed_value - initial_value * SPECTRUM_DECAY).abs() < 0.01);
    }
}
