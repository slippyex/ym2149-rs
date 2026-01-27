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

                // Step calculation from Arkos Tracker 3 PsgStreamGenerator.cpp:
                // step = (samplePlayerFrequency / sampleRate) * (pitchHz / referenceFrequency)
                // - sample_player_frequency_hz: PSG hardware playback rate (e.g., 8000-11025 Hz)
                // - pitch_hz: target frequency from note (using noteReference=12 for samples)
                // - reference_frequency_hz: tuning reference (typically 440 Hz)
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
                    // Sample already playing - update params but only reset position if play_from_start
                    active.update_from_params(params, step);
                    if params.play_from_start {
                        active.position = 0.0;
                    }
                } else if params.play_from_start {
                    // No sample playing - only start if play_from_start is true (AT3 behavior)
                    self.active = Some(ActiveSample::new(params, step));
                }
                // If no active sample and play_from_start is false, do nothing (AT3 behavior)
            }
        }
    }

    /// Get the next sample value (scaled 0.0-6.0 for PSG mixer drum override) and advance position.
    /// Returns None if no active sample or sample ended.
    pub fn next_sample_for_override(&mut self) -> Option<f32> {
        let active = self.active.as_mut()?;

        if active.data.is_empty() {
            self.active = None;
            return None;
        }

        let idx = active.position as usize;
        if idx >= active.loop_end || idx >= active.data.len() {
            if active.looping {
                active.position = active.loop_start as f32;
                // Return 0 for this sample, will get actual value on next call
                return Some(0.0);
            } else {
                self.active = None;
                return None;
            }
        }

        // Get sample value (data is -1.0 to 1.0, convert to 0.0-4.0 for PSG mixer)
        // Using 2.0 multiplier for better balance with PSG audio levels
        let value = (active.data[idx] * active.gain + 1.0) * 2.0;
        let value = value.clamp(0.0, 4.0);

        active.advance_position();
        if active.step == 0.0 {
            self.active = None;
        }

        Some(value)
    }

    /// Mixes the active sample into the output buffer segment.
    ///
    /// # Arguments
    ///
    /// * `segment` - Mutable slice to mix samples into (additive)
    #[allow(dead_code)]
    pub fn mix_into(&mut self, segment: &mut [f32]) {
        let Some(active) = self.active.as_mut() else {
            return;
        };

        let mut stop = false;
        for sample in segment.iter_mut() {
            if active.data.is_empty() {
                stop = true;
                break;
            }

            let idx = active.position as usize;
            if idx >= active.loop_end || idx >= active.data.len() {
                if active.looping {
                    active.position = active.loop_start as f32;
                    // Don't output this sample, just wrap and continue to next
                    // The advance_position call is skipped, position stays at loop_start
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

        // AT3 behavior: when play_from_start is true, start from position 0
        // When not playing from start, continue from loop_start
        let initial_position = if params.play_from_start { 0.0 } else { loop_start as f32 };

        Self {
            data: Arc::clone(&params.data),
            position: initial_position,
            loop_start,
            loop_end,
            looping: params.looping,
            step,
            gain: Self::compute_gain(params),
            high_priority: params.high_priority,
        }
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

/// Converts a note number to frequency in Hz using equal temperament.
///
/// This is the general-purpose frequency calculation used for PSG notes.
/// Formula: (referenceFrequency / 32.0) * 2^((note - noteReference) / 12.0)
///
/// # Arguments
///
/// * `reference_frequency` - Reference frequency (e.g., 440.0 for A4)
/// * `note` - Note number (0 = C-3, 36 = C0, 69 = A4, 72 = C5, etc.)
/// * `note_reference` - Reference note (default 9 for PSG, 12 for samples)
fn note_frequency_internal(reference_frequency: f32, note: i32, note_reference: i32) -> f32 {
    if note < 0 || reference_frequency <= 0.0 {
        return 0.0;
    }

    // Formula from Arkos Tracker 3: (referenceFrequency / 32.0) * 2^((note - noteReference) / 12.0)
    let freq = (reference_frequency as f64 / 32.0)
        * 2.0_f64.powf((note - note_reference) as f64 / 12.0);
    freq as f32
}

/// Converts a note number to frequency in Hz for sample playback.
///
/// Uses noteReference=12, which means C5 (note 72) = 440 Hz with reference 440.
/// This matches how other trackers handle samples, where C-5 plays the sample "as is".
///
/// For digidrums with digidrum_note=72 and reference=440, this returns 440 Hz,
/// meaning the sample plays at 1:1 speed ratio.
pub fn sample_note_frequency(reference_frequency: f32, note: i32) -> f32 {
    note_frequency_internal(reference_frequency, note, 12)
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
