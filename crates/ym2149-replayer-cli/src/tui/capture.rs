//! Sample capture buffer for waveform and spectrum visualization.
//!
//! This module wraps the shared visualization utilities from `ym2149_common`
//! and provides a thread-safe interface for the TUI.
//!
//! Supports up to 4 PSGs (12 channels) for multi-PSG configurations like AKS.
//! Also tracks SID voice and DigiDrum effects for special visualization.
//! Provides smoothed data and velocity tracking for dynamic visualization.

use ym2149_common::visualization::{
    MAX_CHANNEL_COUNT, MAX_PSG_COUNT, SPECTRUM_BINS, SpectrumAnalyzer, WaveformSynthesizer,
};

/// Smoothing factor for spectrum velocity (0.0-1.0, higher = more smoothing)
const VELOCITY_SMOOTHING: f32 = 0.7;

/// Capture buffer for waveform and spectrum data.
///
/// Wraps the shared `WaveformSynthesizer` and `SpectrumAnalyzer` from
/// `ym2149_common::visualization` to provide a unified interface for the TUI.
///
/// Supports up to 4 PSGs (12 channels total) for multi-PSG configurations.
/// Tracks SID/DigiDrum/Buzz effects for special visualization handling.
/// Also tracks spectrum velocity (rate of change) for dynamic brightness.
pub struct CaptureBuffer {
    /// Waveform synthesizer from shared library.
    waveform: WaveformSynthesizer,
    /// Spectrum analyzer from shared library.
    spectrum: SpectrumAnalyzer,
    /// Current PSG count.
    psg_count: usize,
    /// SID voice effect active per channel (bypasses normal waveform).
    sid_active: [bool; MAX_CHANNEL_COUNT],
    /// DigiDrum effect active per channel (sample playback).
    drum_active: [bool; MAX_CHANNEL_COUNT],
    /// Buzz/Envelope effect active per channel (LFSR + envelope).
    buzz_active: [bool; MAX_CHANNEL_COUNT],
    /// Previous spectrum values for velocity calculation.
    prev_spectrum: [[f32; SPECTRUM_BINS]; MAX_CHANNEL_COUNT],
    /// Smoothed spectrum velocity (rate of change) per channel/bin.
    spectrum_velocity: [[f32; SPECTRUM_BINS]; MAX_CHANNEL_COUNT],
}

impl CaptureBuffer {
    /// Create a new capture buffer.
    pub fn new() -> Self {
        Self {
            waveform: WaveformSynthesizer::new(),
            spectrum: SpectrumAnalyzer::new(),
            psg_count: 1,
            sid_active: [false; MAX_CHANNEL_COUNT],
            drum_active: [false; MAX_CHANNEL_COUNT],
            buzz_active: [false; MAX_CHANNEL_COUNT],
            prev_spectrum: [[0.0; SPECTRUM_BINS]; MAX_CHANNEL_COUNT],
            spectrum_velocity: [[0.0; SPECTRUM_BINS]; MAX_CHANNEL_COUNT],
        }
    }

    /// Update spectrum and waveforms from multiple PSG register banks.
    ///
    /// This uses register-based frequencies instead of FFT for accurate
    /// note-aligned spectrum visualization. Waveforms are synthesized from
    /// the channel frequencies and amplitudes.
    ///
    /// Also updates SID/DigiDrum/Buzz status and calculates spectrum velocity.
    pub fn update_from_registers(
        &mut self,
        register_banks: &[[u8; 16]],
        psg_count: usize,
        sid_active: &[bool; MAX_CHANNEL_COUNT],
        drum_active: &[bool; MAX_CHANNEL_COUNT],
    ) {
        let count = psg_count.clamp(1, MAX_PSG_COUNT);
        self.psg_count = count;
        self.sid_active = *sid_active;
        self.drum_active = *drum_active;

        // Detect buzz/envelope mode from registers
        // Buzz is active when envelope_enabled (bit 4 of amplitude register) is set
        self.buzz_active = [false; MAX_CHANNEL_COUNT];
        for (psg_idx, regs) in register_banks.iter().enumerate().take(count) {
            for ch in 0..3 {
                let global_ch = psg_idx * 3 + ch;
                let amp_reg = regs[8 + ch];
                let envelope_enabled = (amp_reg & 0x10) != 0;
                self.buzz_active[global_ch] = envelope_enabled;
            }
        }

        self.waveform.update_multi_psg(register_banks, count);
        self.spectrum.update_multi_psg(register_banks, count);

        // Calculate spectrum velocity (rate of change) for dynamic brightness
        let channel_count = count * 3;
        for ch in 0..channel_count {
            let current = self.spectrum.channel_spectrum(ch);
            for (bin, &cur_val) in current.iter().enumerate() {
                // Calculate absolute change
                let delta = (cur_val - self.prev_spectrum[ch][bin]).abs();
                // Smooth the velocity (exponential moving average)
                self.spectrum_velocity[ch][bin] =
                    VELOCITY_SMOOTHING * self.spectrum_velocity[ch][bin]
                        + (1.0 - VELOCITY_SMOOTHING) * delta * 3.0; // Scale up for visibility
                // Store current for next frame
                self.prev_spectrum[ch][bin] = cur_val;
            }
        }
    }

    /// Get waveform samples for a channel (0-11 for multi-PSG).
    pub fn waveform(&self, channel: usize) -> &std::collections::VecDeque<f32> {
        self.waveform.channel_waveform(channel)
    }

    /// Get spectrum for a specific channel (0-11 for multi-PSG).
    pub fn spectrum_channel(&self, channel: usize) -> &[f32; SPECTRUM_BINS] {
        self.spectrum.channel_spectrum(channel)
    }

    /// Get the current channel count.
    pub fn channel_count(&self) -> usize {
        self.psg_count * 3
    }

    /// Check if SID voice effect is active on a channel.
    pub fn is_sid_active(&self, channel: usize) -> bool {
        self.sid_active.get(channel).copied().unwrap_or(false)
    }

    /// Check if DigiDrum effect is active on a channel.
    pub fn is_drum_active(&self, channel: usize) -> bool {
        self.drum_active.get(channel).copied().unwrap_or(false)
    }

    /// Check if Buzz/Envelope effect is active on a channel.
    pub fn is_buzz_active(&self, channel: usize) -> bool {
        self.buzz_active.get(channel).copied().unwrap_or(false)
    }

    /// Get spectrum velocity (rate of change) for a channel/bin.
    /// Higher values indicate more dynamic/changing spectrum.
    pub fn spectrum_velocity(&self, channel: usize, bin: usize) -> f32 {
        self.spectrum_velocity
            .get(channel)
            .and_then(|ch| ch.get(bin))
            .copied()
            .unwrap_or(0.0)
            .clamp(0.0, 1.0)
    }

    /// Get mono output waveform (sum of all active channels).
    pub fn mono_output(&self) -> std::collections::VecDeque<f32> {
        let channel_count = self.channel_count();
        if channel_count == 0 {
            return std::collections::VecDeque::new();
        }

        // Get the length of the shortest waveform
        let len = (0..channel_count)
            .map(|ch| self.waveform.channel_waveform(ch).len())
            .min()
            .unwrap_or(0);

        if len == 0 {
            return std::collections::VecDeque::new();
        }

        // Sum all channels
        let mut mono = std::collections::VecDeque::with_capacity(len);
        for i in 0..len {
            let sum: f32 = (0..channel_count)
                .map(|ch| self.waveform.channel_waveform(ch).get(i).copied().unwrap_or(0.0))
                .sum();
            // Normalize by channel count to prevent clipping
            mono.push_back(sum / channel_count as f32);
        }
        mono
    }
}

impl Default for CaptureBuffer {
    fn default() -> Self {
        Self::new()
    }
}
