//! Sample voice mixing for digi-drum and sample playback.
//!
//! This module handles the playback of PCM samples alongside PSG audio,
//! supporting features like looping, pitch shifting, and volume control.

use crate::channel_player::{SampleCommand, SamplePlaybackParams};
use std::sync::Arc;

/// Mixes sample playback into the audio output.
///
/// Each channel can have an active sample that is mixed into the final audio.
/// Samples support looping, pitch adjustment, and amplitude control.
#[derive(Default, Clone)]
pub(crate) struct SampleVoiceMixer {
    active: Option<ActiveSample>,
}

impl SampleVoiceMixer {
    /// Applies a sample command to this voice.
    ///
    /// # Arguments
    ///
    /// * `command` - The sample command (None, Stop, or Play)
    /// * `output_sample_rate` - Target sample rate for resampling
    pub fn apply_command(&mut self, command: &SampleCommand, output_sample_rate: f32) {
        match command {
            SampleCommand::None => {}
            SampleCommand::Stop => self.active = None,
            SampleCommand::Play(params) => {
                if params.pitch_hz <= 0.0
                    || params.reference_frequency_hz <= 0.0
                    || params.sample_player_frequency_hz <= 0.0
                    || params.data.is_empty()
                {
                    self.active = None;
                    return;
                }

                let mut step = (params.sample_player_frequency_hz / output_sample_rate)
                    * (params.pitch_hz / params.reference_frequency_hz);
                if !step.is_finite() || step <= 0.0 {
                    step = 0.0;
                }

                if step == 0.0 {
                    self.active = None;
                    return;
                }

                // High priority samples interrupt low priority ones
                if let Some(active) = self.active.as_ref()
                    && active.high_priority
                    && !params.high_priority
                {
                    return;
                }

                if let Some(active) = self.active.as_mut() {
                    active.update_from_params(params, step);
                    if params.play_from_start {
                        active.position = params.loop_start as f32;
                    }
                } else {
                    self.active = Some(ActiveSample::new(params, step));
                }
            }
        }
    }

    /// Mixes the active sample into the output buffer segment.
    ///
    /// # Arguments
    ///
    /// * `segment` - Mutable slice to mix samples into (additive)
    pub fn mix_into(&mut self, segment: &mut [f32]) {
        if self.active.is_none() {
            return;
        }

        let mut stop = false;
        {
            let active = self.active.as_mut().unwrap();
            for sample in segment.iter_mut() {
                if active.data.is_empty() {
                    stop = true;
                    break;
                }

                let idx = active.position as usize;
                if idx >= active.loop_end || idx >= active.data.len() {
                    if active.looping {
                        active.position = active.loop_start as f32;
                        continue;
                    } else {
                        stop = true;
                        break;
                    }
                }

                *sample += active.data[idx] * active.gain;
                active.advance_position();
                if active.step == 0.0 {
                    stop = true;
                    break;
                }
            }
        }

        if stop {
            self.active = None;
        }
    }
}

/// Active sample playback state.
#[derive(Clone)]
pub(crate) struct ActiveSample {
    data: Arc<Vec<f32>>,
    pub(crate) position: f32,
    loop_start: usize,
    loop_end: usize,
    looping: bool,
    step: f32,
    gain: f32,
    pub(crate) high_priority: bool,
}

impl ActiveSample {
    /// Creates a new active sample from playback parameters.
    pub fn new(params: &SamplePlaybackParams, step: f32) -> Self {
        let mut loop_end = params.loop_end.saturating_add(1);
        loop_end = loop_end.min(params.data.len());
        if loop_end == 0 {
            loop_end = params.data.len();
        }
        let mut loop_start = params.loop_start.min(loop_end.saturating_sub(1));
        if loop_start >= loop_end {
            loop_start = 0;
        }

        let mut instance = Self {
            data: Arc::clone(&params.data),
            position: loop_start as f32,
            loop_start,
            loop_end,
            looping: params.looping,
            step,
            gain: Self::compute_gain(params),
            high_priority: params.high_priority,
        };
        if !params.play_from_start {
            instance.position = loop_start as f32;
        }
        instance
    }

    /// Updates this sample with new parameters (for retriggering).
    pub fn update_from_params(&mut self, params: &SamplePlaybackParams, step: f32) {
        self.data = Arc::clone(&params.data);
        let mut loop_end = params.loop_end.saturating_add(1).min(self.data.len());
        if loop_end == 0 {
            loop_end = self.data.len();
        }
        let mut loop_start = params.loop_start.min(loop_end.saturating_sub(1));
        if loop_start >= loop_end {
            loop_start = 0;
        }
        self.loop_start = loop_start;
        self.loop_end = loop_end;
        self.looping = params.looping;
        self.step = step;
        self.gain = Self::compute_gain(params);
        self.high_priority = params.high_priority;
    }

    fn compute_gain(params: &SamplePlaybackParams) -> f32 {
        let volume = (params.volume as f32 / 15.0).clamp(0.0, 1.0);
        volume * params.amplification
    }

    /// Advances the playback position by one step.
    pub fn advance_position(&mut self) {
        let mut next_position = self.position + self.step;
        if next_position.is_nan() || !next_position.is_finite() {
            self.step = 0.0;
            return;
        }

        while next_position as usize >= self.loop_end {
            if self.looping {
                let overflow = next_position - self.loop_end as f32;
                next_position = self.loop_start as f32 + overflow;
            } else {
                self.step = 0.0;
                break;
            }
        }

        self.position = next_position;
    }
}

/// Converts a note number to frequency in Hz.
///
/// Uses equal temperament with the given reference frequency (typically 440 Hz for A4).
///
/// # Arguments
///
/// * `reference_frequency` - Reference frequency (e.g., 440.0 for A4)
/// * `note` - Note number (0 = C-3, 36 = C0, 72 = C3, etc.)
pub fn note_frequency(reference_frequency: f32, note: i32) -> f32 {
    if note < 0 {
        return 0.0;
    }

    const START_OCTAVE: i32 = -3;
    const NOTES_IN_OCTAVE: i32 = 12;

    let octave = (note / NOTES_IN_OCTAVE) + START_OCTAVE;
    let note_in_octave = (note % NOTES_IN_OCTAVE) + 1;

    ((reference_frequency as f64)
        * 2.0_f64.powf(octave as f64 + ((note_in_octave as f64 - 10.0) / 12.0))) as f32
}

/// Hardware envelope state tracking to avoid unwanted retriggering.
#[derive(Clone, Copy)]
pub(crate) struct HardwareEnvelopeState {
    pub last_shape: u8,
}

impl Default for HardwareEnvelopeState {
    fn default() -> Self {
        Self { last_shape: 0xFF }
    }
}
