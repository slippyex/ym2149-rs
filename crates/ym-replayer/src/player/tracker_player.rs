//! YM Tracker format player (YMT1/YMT2)
//!
//! Handles sample-based playback for YM Tracker formats, which are different from
//! the register-based YM2-YM6 formats.

/// Fixed-point precision for sample position tracking
const YM_TRACKER_PRECISION: u32 = 16;

/// YM Tracker format version
#[derive(Clone, Copy, Debug)]
pub(crate) enum TrackerFormat {
    Ymt1,
    Ymt2,
}

/// Single line of tracker data (per voice per frame)
#[derive(Clone, Copy)]
pub(crate) struct TrackerLine {
    pub note_on: u8,
    pub volume: u8,
    pub freq_high: u8,
    pub freq_low: u8,
}

/// Tracker sample (digi-drum) data
#[derive(Clone)]
pub(crate) struct TrackerSample {
    pub data: Vec<u8>,
    pub repeat_len: usize,
}

/// State for a single tracker voice
#[derive(Clone)]
struct TrackerVoiceState {
    sample_index: Option<usize>,
    sample_pos: u32,
    sample_freq: u32,
    sample_volume: u8,
    loop_enabled: bool,
    running: bool,
    sample_inc: u32,
}

impl TrackerVoiceState {
    fn new() -> Self {
        TrackerVoiceState {
            sample_index: None,
            sample_pos: 0,
            sample_freq: 0,
            sample_volume: 0,
            loop_enabled: false,
            running: false,
            sample_inc: 0,
        }
    }
}

/// Complete tracker playback state
pub(crate) struct TrackerState {
    voices: Vec<TrackerVoiceState>,
    lines: Vec<TrackerLine>,
    samples: Vec<TrackerSample>,
    volume_table: Vec<i16>,
    freq_shift: u8,
    nb_voice: usize,
    pub(crate) total_frames: usize,
    pub(crate) loop_frame: usize,
    pub(crate) loop_enabled: bool,
    pub(crate) player_rate: u16,
    pub(crate) current_frame: usize,
    pub(crate) samples_until_update: f64,
    pub(crate) samples_per_step: f64,
    sample_rate: u32,
}

impl TrackerState {
    /// Create new tracker state
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        nb_voice: usize,
        player_rate: u16,
        total_frames: usize,
        loop_frame: usize,
        loop_enabled: bool,
        freq_shift: u8,
        samples: Vec<TrackerSample>,
        lines: Vec<TrackerLine>,
        sample_rate: u32,
    ) -> Self {
        let clamped_voice_count = nb_voice.max(1);
        let mut scale = ((256 * 100) / (clamped_voice_count * 100)) as i32;
        if scale == 0 {
            scale = 1;
        }

        // Pre-compute volume scaling table
        let mut volume_table = Vec::with_capacity(64 * 256);
        for volume in 0..64 {
            for sample in -128..128 {
                let value = (sample * scale * volume) / 64;
                volume_table.push(value as i16);
            }
        }

        let voices = (0..nb_voice).map(|_| TrackerVoiceState::new()).collect();
        let samples_per_step = if player_rate == 0 {
            sample_rate as f64
        } else {
            sample_rate as f64 / f64::from(player_rate)
        };

        TrackerState {
            voices,
            lines,
            samples,
            volume_table,
            freq_shift,
            nb_voice,
            total_frames,
            loop_frame,
            loop_enabled,
            player_rate,
            current_frame: 0,
            samples_until_update: 0.0,
            samples_per_step,
            sample_rate,
        }
    }

    /// Reset playback to beginning
    pub(crate) fn reset(&mut self) {
        self.current_frame = 0;
        self.samples_until_update = 0.0;
        for voice in &mut self.voices {
            *voice = TrackerVoiceState::new();
        }
    }

    /// Compute sample increment for given frequency
    fn compute_sample_inc(&self, sample_freq: u32) -> u32 {
        if sample_freq == 0 || self.sample_rate == 0 {
            return 0;
        }
        let mut step = (sample_freq as u64) << YM_TRACKER_PRECISION;
        step <<= u32::from(self.freq_shift.min(15));
        (step / self.sample_rate as u64) as u32
    }

    /// Advance to next frame, returns false if playback ended
    pub(crate) fn advance_frame(&mut self) -> bool {
        if self.total_frames == 0 {
            return false;
        }

        if self.current_frame >= self.total_frames {
            if self.loop_enabled && self.loop_frame < self.total_frames {
                self.current_frame = self.loop_frame;
            } else {
                return false;
            }
        }

        let start = self.current_frame.saturating_mul(self.nb_voice);
        for voice_index in 0..self.nb_voice {
            let line = self.lines[start + voice_index];
            let freq = ((line.freq_high as u32) << 8) | (line.freq_low as u32);
            let sample_inc = if freq != 0 {
                self.compute_sample_inc(freq)
            } else {
                0
            };
            let voice = &mut self.voices[voice_index];

            if freq != 0 {
                voice.sample_freq = freq;
                voice.sample_volume = line.volume & 0x3F;
                voice.loop_enabled = (line.volume & 0x40) != 0;

                if line.note_on != 0xFF {
                    let sample_idx = line.note_on as usize;
                    if let Some(sample) = self.samples.get(sample_idx) {
                        if !sample.data.is_empty() {
                            voice.sample_index = Some(sample_idx);
                            voice.sample_pos = 0;
                            voice.running = true;
                        } else {
                            voice.sample_index = None;
                            voice.running = false;
                        }
                    } else {
                        voice.sample_index = None;
                        voice.running = false;
                    }
                } else if voice.sample_index.is_none() {
                    voice.running = false;
                }

                voice.sample_inc = sample_inc;
                if voice.sample_inc == 0 && voice.sample_index.is_none() {
                    voice.running = false;
                } else if voice.sample_index.is_some() {
                    voice.running = true;
                }
            } else {
                voice.sample_freq = 0;
                voice.running = false;
                voice.sample_inc = 0;
            }
        }

        self.current_frame += 1;
        true
    }

    /// Jump to a specific tracker frame (clamped) and reset voice state.
    pub(crate) fn seek_frame(&mut self, frame: usize) {
        if self.total_frames == 0 {
            self.current_frame = 0;
        } else {
            self.current_frame = frame.min(self.total_frames.saturating_sub(1));
        }
        self.samples_until_update = 0.0;
        for voice in &mut self.voices {
            *voice = TrackerVoiceState::new();
        }
    }

    /// Mix all active voices into a single sample
    pub(crate) fn mix_sample(&mut self) -> f32 {
        let mut accumulator: i32 = 0;

        for voice in &mut self.voices {
            if !voice.running {
                continue;
            }

            let sample_idx = match voice.sample_index {
                Some(idx) => idx,
                None => {
                    voice.running = false;
                    continue;
                }
            };

            let sample = match self.samples.get(sample_idx) {
                Some(sample) if !sample.data.is_empty() => sample,
                _ => {
                    voice.running = false;
                    continue;
                }
            };

            let sample_end = (sample.data.len() as u32) << YM_TRACKER_PRECISION;
            let mut pos = voice.sample_pos;

            // Handle sample end/loop
            if pos >= sample_end {
                if voice.loop_enabled && sample.repeat_len > 0 {
                    let rep = (sample.repeat_len as u32) << YM_TRACKER_PRECISION;
                    if rep > 0 {
                        while pos >= sample_end {
                            pos = pos.saturating_sub(rep);
                            if rep == 0 {
                                break;
                            }
                        }
                    } else {
                        voice.running = false;
                        continue;
                    }
                } else {
                    voice.running = false;
                    continue;
                }
            }

            // Linear interpolation between samples
            let index = (pos >> YM_TRACKER_PRECISION) as usize;
            let frac = pos & ((1 << YM_TRACKER_PRECISION) - 1);
            let table_offset = (voice.sample_volume as usize & 63) * 256;

            let base_value = self.volume_table[table_offset + sample.data[index] as usize] as i32;
            let blended = if frac != 0 && index + 1 < sample.data.len() {
                let next_value =
                    self.volume_table[table_offset + sample.data[index + 1] as usize] as i32;
                base_value + (((next_value - base_value) * frac as i32) >> YM_TRACKER_PRECISION)
            } else {
                base_value
            };

            accumulator += blended;

            // Advance position
            let mut new_pos = pos.wrapping_add(voice.sample_inc);
            if new_pos >= sample_end {
                if voice.loop_enabled && sample.repeat_len > 0 {
                    let rep = (sample.repeat_len as u32) << YM_TRACKER_PRECISION;
                    if rep > 0 {
                        while new_pos >= sample_end {
                            new_pos = new_pos.saturating_sub(rep);
                            if rep == 0 {
                                break;
                            }
                        }
                    } else {
                        voice.running = false;
                        voice.sample_pos = sample_end;
                        continue;
                    }
                } else {
                    voice.running = false;
                    voice.sample_pos = sample_end;
                    continue;
                }
            }

            voice.sample_pos = new_pos;
        }

        (accumulator as f32 / 32768.0).clamp(-1.0, 1.0)
    }
}

/// Deinterleave tracker bytes from interleaved to linear format
pub(crate) fn deinterleave_tracker_bytes(
    bytes: &[u8],
    nb_voice: usize,
    total_frames: usize,
) -> Vec<u8> {
    let step = nb_voice * 4;
    let mut output = vec![0u8; bytes.len()];
    let mut src_index = 0;
    for column in 0..step {
        let mut dest = column;
        for _ in 0..total_frames {
            output[dest] = bytes[src_index];
            src_index += 1;
            dest += step;
        }
    }
    output
}
