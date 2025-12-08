//! GPU uniform buffer resources for visualization shaders.

use bevy::prelude::*;
use ym2149_common::visualization::{SPECTRUM_BINS, SpectrumAnalyzer, WaveformSynthesizer};

/// Buffer storing oscilloscope samples ready to upload to GPU uniforms.
///
/// Each entry is `[amplitude_a, amplitude_b, amplitude_c]` for one time sample.
#[derive(Resource, Default, Clone)]
pub struct OscilloscopeUniform(
    /// Per-sample amplitude values for channels A, B, C.
    pub Vec<[f32; 3]>,
);

/// Buffer storing spectrum magnitudes ready for GPU uniforms.
///
/// Each entry contains frequency bin magnitudes for one channel (96 bins = 8 octaves × 12 semitones).
#[derive(Resource, Default, Clone)]
pub struct SpectrumUniform(
    /// Per-channel array of frequency bin magnitudes.
    pub Vec<[f32; SPECTRUM_BINS]>,
);

/// State for register-based waveform synthesis.
///
/// This generates oscilloscope waveforms directly from YM2149 register state
/// rather than from audio samples, ensuring visualization even when using
/// digidrums or STE-DAC samples that bypass the PSG.
///
/// Wraps the shared implementation from `ym2149_common::visualization`.
#[derive(Resource, Clone)]
pub struct RegisterWaveformState {
    /// Waveform synthesizer from shared library.
    pub synthesizer: WaveformSynthesizer,
    /// Spectrum analyzer from shared library.
    pub spectrum: SpectrumAnalyzer,
}

impl Default for RegisterWaveformState {
    fn default() -> Self {
        Self {
            synthesizer: WaveformSynthesizer::new(),
            spectrum: SpectrumAnalyzer::new(),
        }
    }
}

impl RegisterWaveformState {
    /// Update waveforms and spectrum from YM2149 channel states.
    pub fn update_from_channel_states(&mut self, channel_states: &ym2149_common::ChannelStates) {
        self.synthesizer.update(channel_states);
        self.spectrum.update(channel_states);
    }

    /// Get waveform samples for oscilloscope display.
    pub fn get_samples(&self) -> Vec<[f32; 3]> {
        self.synthesizer.get_samples()
    }

    /// Get spectrum bins for the first 3 channels (single PSG).
    pub fn get_spectrum(&self) -> [[f32; SPECTRUM_BINS]; 3] {
        let all = self.spectrum.all_channel_spectrums();
        [all[0], all[1], all[2]]
    }

    /// Compute high frequency ratio for a channel.
    pub fn high_freq_ratio(&self, channel: usize) -> f32 {
        self.spectrum.high_freq_ratio(channel)
    }
}
