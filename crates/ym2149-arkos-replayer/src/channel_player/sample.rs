//! Sample voice state and conversion to playback parameters.

use super::output::SamplePlaybackParams;
use std::sync::Arc;

/// Internal sample voice state for tracking active sample playback
#[derive(Clone)]
pub(super) struct SampleVoiceState {
    pub data: Arc<Vec<f32>>,
    pub loop_start: usize,
    pub loop_end: usize,
    pub looping: bool,
    pub amplification: f32,
    pub pitch_hz: f32,
    pub volume_4bits: u8,
    /// Reference frequency in Hz (tuning reference, typically 440 Hz)
    pub reference_frequency_hz: f32,
    /// PSG sample player frequency in Hz (hardware playback rate)
    pub sample_player_frequency_hz: f32,
    pub high_priority: bool,
}

impl SampleVoiceState {
    /// Convert to playback parameters for output
    pub fn to_params(&self, play_from_start: bool) -> SamplePlaybackParams {
        SamplePlaybackParams {
            data: Arc::clone(&self.data),
            loop_start: self.loop_start,
            loop_end: self.loop_end,
            looping: self.looping,
            pitch_hz: self.pitch_hz,
            amplification: self.amplification,
            volume: self.volume_4bits,
            sample_player_frequency_hz: self.sample_player_frequency_hz,
            reference_frequency_hz: self.reference_frequency_hz,
            play_from_start,
            high_priority: self.high_priority,
        }
    }
}
