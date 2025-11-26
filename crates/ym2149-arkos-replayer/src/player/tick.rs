//! Tick processing and song advancement.
//!
//! This module handles the core playback timing:
//! - Processing individual ticks (pattern update rate, typically 50Hz)
//! - Advancing through lines and positions
//! - Speed track handling
//! - Loop boundary detection

use std::sync::Arc;

use super::psg_output::write_frames_to_psg;
use super::sample_voice::{HardwareEnvelopeState, SampleVoiceMixer, note_frequency};
use crate::channel_player::{ChannelFrame, ChannelPlayer, SampleCommand, SamplePlaybackParams};
use crate::effect_context::EffectContext;
use crate::format::{AksSong, InstrumentType, Subsong};
use ym2149::ym2149::PsgBank;

/// Tick processing context containing all mutable state needed for a tick.
pub(crate) struct TickContext<'a> {
    pub song: &'a Arc<AksSong>,
    pub effect_context: &'a EffectContext,
    pub psg_bank: &'a mut PsgBank,
    pub subsong_index: usize,
    pub channel_players: &'a mut [ChannelPlayer],
    pub current_position: &'a mut usize,
    pub current_line: &'a mut usize,
    pub current_speed: &'a mut u8,
    pub current_tick: &'a mut u8,
    pub sample_voices: &'a mut [SampleVoiceMixer],
    pub hardware_envelope_state: &'a mut [HardwareEnvelopeState],
    pub output_sample_rate: f32,
}

impl TickContext<'_> {
    /// Process one tick of playback.
    pub fn process_tick(&mut self) {
        self.run_tick(|_| {});
    }

    /// Process tick with an observer callback for frame capture.
    pub fn run_tick<F>(&mut self, mut observer: F)
    where
        F: FnMut(&[ChannelFrame]),
    {
        let is_first_tick = *self.current_tick == 0;

        if is_first_tick {
            self.apply_speed_track();
        }

        let position_count = self.song.subsongs[self.subsong_index].positions.len();
        let (loop_start, past_end_position) = self.loop_bounds(position_count);

        if is_first_tick && position_count > 0 && *self.current_position >= position_count {
            *self.current_position = loop_start;
            *self.current_line = 0;
        }

        let frames = self.build_frames(position_count, is_first_tick);

        self.apply_frame_samples(&frames);
        observer(&frames);

        if is_first_tick
            && let Some(event_value) = self.read_special_track_value(false)
            && event_value > 0
        {
            self.trigger_event_sample(event_value as usize);
        }

        write_frames_to_psg(self.psg_bank, &frames, self.hardware_envelope_state);

        self.advance_song(loop_start, past_end_position);
    }

    fn build_frames(&mut self, position_count: usize, is_first_tick: bool) -> Vec<ChannelFrame> {
        let still_within_line = true;
        let mut frames = Vec::with_capacity(self.channel_players.len());

        if is_first_tick {
            for (channel_idx, channel_player) in self.channel_players.iter_mut().enumerate() {
                if *self.current_position >= position_count {
                    frames.push(channel_player.play_frame(
                        None,
                        0,
                        is_first_tick,
                        still_within_line,
                    ));
                    continue;
                }

                let (cell, transposition) = {
                    let subsong = &self.song.subsongs[self.subsong_index];
                    resolve_cell(
                        subsong,
                        *self.current_position,
                        *self.current_line,
                        channel_idx,
                    )
                };

                if let Some(context) = self
                    .effect_context
                    .line_context(*self.current_position, channel_idx, *self.current_line)
                    .cloned()
                {
                    channel_player.apply_line_context(&context);
                }

                let frame = channel_player.play_frame(
                    cell,
                    transposition,
                    is_first_tick,
                    still_within_line,
                );
                frames.push(frame);
            }
        } else {
            for channel_player in self.channel_players.iter_mut() {
                frames.push(channel_player.play_frame(None, 0, is_first_tick, still_within_line));
            }
        }

        frames
    }

    fn apply_frame_samples(&mut self, frames: &[ChannelFrame]) {
        for (channel_idx, frame) in frames.iter().enumerate() {
            if let Some(voice) = self.sample_voices.get_mut(channel_idx) {
                voice.apply_command(&frame.sample, self.output_sample_rate);
            }
        }
    }

    fn loop_bounds(&self, position_count: usize) -> (usize, usize) {
        if position_count == 0 {
            return (0, 0);
        }

        let subsong = &self.song.subsongs[self.subsong_index];
        let loop_start = subsong
            .loop_start_position
            .min(position_count.saturating_sub(1));

        let past_end = subsong.end_position.saturating_add(1).min(position_count);
        let past_end_position = past_end.max(loop_start.saturating_add(1));

        (loop_start, past_end_position)
    }

    fn advance_song(&mut self, loop_start: usize, past_end_position: usize) {
        *self.current_tick += 1;

        if *self.current_tick >= *self.current_speed {
            *self.current_tick = 0;
            *self.current_line += 1;

            let pattern_height = self.get_pattern_height();
            if *self.current_line >= pattern_height {
                *self.current_line = 0;
                *self.current_position += 1;

                if past_end_position > 0 && *self.current_position >= past_end_position {
                    *self.current_position = loop_start;
                }
            }
        }
    }

    fn get_pattern_height(&self) -> usize {
        let subsong = &self.song.subsongs[self.subsong_index];
        if *self.current_position < subsong.positions.len() {
            subsong.positions[*self.current_position].height
        } else {
            64 // Default fallback
        }
    }

    fn apply_speed_track(&mut self) {
        if let Some(value) = self.read_special_track_value(true) {
            let clamped = value.clamp(1, u8::MAX as i32) as u8;
            *self.current_speed = clamped;
        }
    }

    fn read_special_track_value(&self, speed: bool) -> Option<i32> {
        read_special_track_value_internal(
            self.song,
            self.subsong_index,
            *self.current_position,
            *self.current_line,
            speed,
        )
    }

    fn trigger_event_sample(&mut self, instrument_index: usize) {
        let subsong = &self.song.subsongs[self.subsong_index];
        if subsong.digi_channel >= self.sample_voices.len() {
            return;
        }

        let instrument = match self.song.instruments.get(instrument_index) {
            Some(instr) => instr,
            None => return,
        };

        if instrument.instrument_type != InstrumentType::Digi {
            return;
        }

        let sample = match &instrument.sample {
            Some(s) => s,
            None => return,
        };

        if sample.data.is_empty() {
            return;
        }

        let channel = subsong.digi_channel;
        let psg_idx = channel / 3;
        let Some(psg) = subsong.psgs.get(psg_idx) else {
            return;
        };

        let pitch_hz = note_frequency(psg.reference_frequency, sample.digidrum_note);
        if pitch_hz <= 0.0 {
            return;
        }

        let params = SamplePlaybackParams {
            data: Arc::clone(&sample.data),
            loop_start: sample.loop_start_index,
            loop_end: sample.end_index,
            looping: sample.is_looping,
            sample_frequency_hz: sample.frequency_hz,
            pitch_hz,
            amplification: sample.amplification_ratio,
            volume: 15,
            sample_player_frequency_hz: psg.sample_player_frequency as f32,
            reference_frequency_hz: psg.reference_frequency,
            play_from_start: true,
            high_priority: true,
        };

        if let Some(voice) = self.sample_voices.get_mut(channel) {
            voice.apply_command(&SampleCommand::Play(params), self.output_sample_rate);
        }
    }
}

/// Resolve a cell from the pattern/track structure.
pub(crate) fn resolve_cell(
    subsong: &Subsong,
    position_idx: usize,
    line_idx: usize,
    channel_idx: usize,
) -> (Option<&crate::format::Cell>, i8) {
    let mut transposition = 0;
    if let Some(position) = subsong.positions.get(position_idx) {
        if let Some(value) = position.transpositions.get(channel_idx) {
            transposition = *value;
        }

        if let Some(pattern) = subsong.patterns.get(position.pattern_index)
            && let Some(track_index) = pattern.track_indexes.get(channel_idx)
            && let Some(track) = subsong.tracks.get(track_index)
        {
            let cell = track.cells.iter().find(|c| c.index == line_idx);
            return (cell, transposition);
        }
    }

    (None, transposition)
}

/// Determine speed for a specific location (used for initialization).
pub(crate) fn determine_speed_for_location(
    song: &AksSong,
    subsong_index: usize,
    position: usize,
    line: usize,
) -> u8 {
    let subsong = &song.subsongs[subsong_index];
    if subsong.positions.is_empty() {
        return subsong.initial_speed;
    }

    let mut speed = subsong.initial_speed;
    let target_position = position.min(subsong.positions.len() - 1);

    for pos_idx in 0..=target_position {
        let height = subsong.positions[pos_idx].height.max(1);
        let target_line = if pos_idx == target_position {
            line.min(height - 1)
        } else {
            height - 1
        };

        if let Some(value) = read_special_track_value_internal(
            &Arc::new(song.clone()),
            subsong_index,
            pos_idx,
            target_line,
            true,
        ) && value > 0
        {
            speed = value.clamp(1, u8::MAX as i32) as u8;
        }
    }

    speed
}

fn read_special_track_value_internal(
    song: &Arc<AksSong>,
    subsong_index: usize,
    position_index: usize,
    line: usize,
    speed: bool,
) -> Option<i32> {
    let subsong = &song.subsongs[subsong_index];

    if position_index >= subsong.positions.len() {
        return None;
    }

    let position = &subsong.positions[position_index];
    if position.pattern_index >= subsong.patterns.len() {
        return None;
    }

    let pattern = &subsong.patterns[position.pattern_index];
    let track_index = if speed {
        pattern.speed_track_index
    } else {
        pattern.event_track_index
    };

    let tracks = if speed {
        &subsong.speed_tracks
    } else {
        &subsong.event_tracks
    };

    let track = tracks.get(&track_index)?;
    track.latest_value(line)
}
