//! Sample capture buffer for waveform and spectrum visualization.
//!
//! This module wraps the shared visualization utilities from `ym2149_common`
//! and provides a thread-safe interface for the TUI.
//!
//! Supports up to 4 PSGs (12 channels) for multi-PSG configurations like AKS.
//! Also tracks SID voice and DigiDrum effects for special visualization.

use ym2149_common::visualization::{
    MAX_CHANNEL_COUNT, MAX_PSG_COUNT, SPECTRUM_BINS, SpectrumAnalyzer, WaveformSynthesizer,
};

/// Capture buffer for waveform and spectrum data.
///
/// Wraps the shared `WaveformSynthesizer` and `SpectrumAnalyzer` from
/// `ym2149_common::visualization` to provide a unified interface for the TUI.
///
/// Supports up to 4 PSGs (12 channels total) for multi-PSG configurations.
/// Tracks SID/DigiDrum effects for special visualization handling.
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
        }
    }

    /// Update spectrum and waveforms from multiple PSG register banks.
    ///
    /// This uses register-based frequencies instead of FFT for accurate
    /// note-aligned spectrum visualization. Waveforms are synthesized from
    /// the channel frequencies and amplitudes.
    ///
    /// Also updates SID/DigiDrum status for special visualization handling.
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

        self.waveform.update_multi_psg(register_banks, count);
        self.spectrum.update_multi_psg(register_banks, count);
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
}

impl Default for CaptureBuffer {
    fn default() -> Self {
        Self::new()
    }
}
