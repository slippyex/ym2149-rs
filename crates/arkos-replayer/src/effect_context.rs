//! Precomputed per-line effect context (volume, pitch, arpeggio)
//!
//! Arkos Tracker keeps effect state (volume, pitch/arpeggio tables, inline arps)
//! between cells. The C++ player queries a background-built context to know what
//! the effective values are at a given location. This module mirrors that logic
//! so ChannelPlayer instances can start each line with the same state.

use std::cmp;

use crate::effects::EffectType;
use crate::error::{ArkosError, Result};
use crate::expression::InlineArpeggio;
use crate::fixed_point::FixedPoint;
use crate::format::{AksSong, Cell, Effect as CellEffect, SpecialCell, SpecialTrack, Track};

/// Per-line context (equivalent to arkostracker::LineContext)
#[derive(Debug, Clone, PartialEq)]
pub struct LineContext {
    pitch_table: Option<usize>,
    arpeggio_table: Option<usize>,
    inline_arpeggio: Option<InlineArpeggio>,
    volume: FixedPoint,
}

impl Default for LineContext {
    fn default() -> Self {
        Self {
            pitch_table: Some(0),
            arpeggio_table: Some(0),
            inline_arpeggio: None,
            volume: FixedPoint::from_int(15),
        }
    }
}

impl LineContext {
    /// Current pitch table index (0 = none)
    pub fn pitch_table(&self) -> Option<usize> {
        self.pitch_table
    }

    /// Current arpeggio table index (0 = none)
    pub fn arpeggio_table(&self) -> Option<usize> {
        self.arpeggio_table
    }

    /// Inline arpeggio expression if any
    pub fn inline_arpeggio(&self) -> Option<&InlineArpeggio> {
        self.inline_arpeggio.as_ref()
    }

    /// Track volume (fixed-point 0..15)
    pub fn volume(&self) -> FixedPoint {
        self.volume
    }

    fn set_pitch_table(&mut self, index: usize) {
        self.pitch_table = Some(index);
    }

    fn set_arpeggio_table(&mut self, index: usize) {
        self.arpeggio_table = Some(index);
        self.inline_arpeggio = None;
    }

    fn set_inline_arpeggio(&mut self, arp: InlineArpeggio) {
        self.inline_arpeggio = Some(arp);
        self.arpeggio_table = None;
    }

    fn set_volume(&mut self, value: FixedPoint) {
        let mut vol = value;
        vol.clamp(15);
        self.volume = vol;
    }

    fn add_volume_delta(&mut self, delta: FixedPoint) {
        let mut vol = self.volume;
        vol += delta;
        vol.clamp(15);
        self.volume = vol;
    }
}

#[derive(Debug, Clone)]
struct TrackContext {
    entries: Vec<(usize, LineContext)>,
}

impl TrackContext {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    fn push_context(&mut self, line_index: usize, context: LineContext) {
        if let Some((last_index, _)) = self.entries.last() {
            debug_assert!(*last_index <= line_index);
            if *last_index == line_index {
                // Should not happen but avoid duplicates by replacing.
                self.entries.pop();
            }
        }
        self.entries.push((line_index, context));
    }

    fn context_at(&self, line_index: usize) -> Option<&LineContext> {
        if self.entries.is_empty() {
            return None;
        }

        match self
            .entries
            .binary_search_by_key(&line_index, |(idx, _)| *idx)
        {
            Ok(pos) => self.entries.get(pos).map(|(_, ctx)| ctx),
            Err(0) => self.entries.first().map(|(_, ctx)| ctx),
            Err(pos) => self.entries.get(pos - 1).map(|(_, ctx)| ctx),
        }
    }
}

#[derive(Debug, Clone)]
struct PositionContext {
    tracks: Vec<TrackContext>,
}

/// Precomputed effect context for a subsong
#[derive(Debug, Clone)]
pub struct EffectContext {
    channel_count: usize,
    positions: Vec<PositionContext>,
}

impl EffectContext {
    /// Build the context for a subsong
    pub fn build(song: &AksSong, subsong_index: usize) -> Result<Self> {
        let subsong = song.subsongs.get(subsong_index).ok_or(ArkosError::InvalidSubsong {
            index: subsong_index,
            available: song.subsongs.len(),
        })?;

        let channel_count = subsong.psgs.len() * 3;
        if channel_count == 0 {
            return Ok(Self {
                channel_count: 0,
                positions: Vec::new(),
            });
        }

        let mut current_contexts = vec![LineContext::default(); channel_count];
        let mut volume_slides = vec![FixedPoint::default(); channel_count];
        let mut positions = Vec::with_capacity(subsong.positions.len());
        let mut global_speed = subsong.initial_speed.max(1);

        for position in &subsong.positions {
            let pattern = subsong
                .patterns
                .get(position.pattern_index)
                .ok_or_else(|| ArkosError::InvalidFormat(format!(
                    "Pattern {} missing for position",
                    position.pattern_index
                )))?;

            let speed_track = subsong.speed_tracks.get(&pattern.speed_track_index);
            let (line_speeds, last_speed) =
                build_line_speeds(speed_track, position.height, global_speed);
            global_speed = last_speed;

            let mut track_contexts = Vec::with_capacity(channel_count);
            let initial_speed_for_track = line_speeds.first().copied().unwrap_or(global_speed);

            for channel_idx in 0..channel_count {
                let track_index = pattern.track_indexes.get(channel_idx).copied();
                let track = track_index.and_then(|idx| subsong.tracks.get(&idx));
                let mut track_context = TrackContext::new();

                parse_track(
                    track,
                    position.height,
                    &line_speeds,
                    initial_speed_for_track,
                    &mut current_contexts[channel_idx],
                    &mut volume_slides[channel_idx],
                    &mut track_context,
                    song,
                );

                track_contexts.push(track_context);
            }

            positions.push(PositionContext {
                tracks: track_contexts,
            });
        }

        Ok(Self {
            channel_count,
            positions,
        })
    }

    /// Get the context for a given channel position
    pub fn line_context(
        &self,
        position_idx: usize,
        channel_idx: usize,
        line_idx: usize,
    ) -> Option<&LineContext> {
        if channel_idx >= self.channel_count {
            return None;
        }
        let position = self.positions.get(position_idx)?;
        if channel_idx >= position.tracks.len() {
            return None;
        }
        position.tracks[channel_idx].context_at(line_idx)
    }
}

#[allow(clippy::too_many_arguments)]
fn parse_track(
    track: Option<&Track>,
    height: usize,
    line_speeds: &[u8],
    fallback_speed: u8,
    current_line_context: &mut LineContext,
    volume_slide: &mut FixedPoint,
    track_context: &mut TrackContext,
    song: &AksSong,
) {
    let mut line_context = current_line_context.clone();
    let cells: &[Cell] = track.map(|t| t.cells.as_slice()).unwrap_or(&[]);
    let mut iter = cells.iter().peekable();

    for line in 0..height {
        while let Some(cell) = iter.peek() {
            if cell.index < line {
                iter.next();
            } else {
                break;
            }
        }

        let cell_opt = if let Some(cell) = iter.peek() {
            if cell.index == line {
                iter.next()
            } else {
                None
            }
        } else {
            None
        };

        if let Some(cell) = cell_opt {
            if cell.note != 255 {
                *volume_slide = FixedPoint::default();
            }
            fill_line_context_from_cell(cell, &mut line_context, volume_slide, song);
        }

        if line == 0 || &line_context != current_line_context {
            *current_line_context = line_context.clone();
            track_context.push_context(line, line_context.clone());
        }

        let speed = line_speeds
            .get(line)
            .copied()
            .unwrap_or(fallback_speed.max(1));
        let delta = volume_slide.mul_int(speed as i32);
        line_context.add_volume_delta(delta);
    }

    *current_line_context = line_context;
}

fn fill_line_context_from_cell(
    cell: &Cell,
    line_context: &mut LineContext,
    volume_slide: &mut FixedPoint,
    song: &AksSong,
) {
    let reset_effect = find_effect(cell, EffectType::Reset);
    if let Some(effect) = reset_effect {
        *line_context = LineContext::default();
        let inverted = clamp_0_15(effect.logical_value);
        let volume = 15 - inverted;
        line_context.set_volume(FixedPoint::from_int(volume as i32));
        *volume_slide = FixedPoint::default();
    }

    if let Some(effect) = find_effect(cell, EffectType::PitchTable) {
        let index = sanitize_expression_index(effect.logical_value, song.pitch_tables.len());
        line_context.set_pitch_table(index);
    }

    let arpeggio_table_effect = find_effect(cell, EffectType::ArpeggioTable);
    if let Some(effect) = arpeggio_table_effect {
        let index = sanitize_expression_index(effect.logical_value, song.arpeggios.len());
        line_context.set_arpeggio_table(index);
    } else if let Some(effect) = find_effect(cell, EffectType::Arpeggio3Notes) {
        apply_inline_arpeggio(effect.logical_value, false, line_context);
    } else if let Some(effect) = find_effect(cell, EffectType::Arpeggio4Notes) {
        apply_inline_arpeggio(effect.logical_value, true, line_context);
    }

    if let Some(effect) = find_effect(cell, EffectType::Volume) {
        let volume = clamp_0_15(effect.logical_value);
        line_context.set_volume(FixedPoint::from_int(volume as i32));
        *volume_slide = FixedPoint::default();
    }

    if let Some(effect) = find_effect(cell, EffectType::VolumeOut) {
        *volume_slide = FixedPoint::from_digits(clamp_u16(effect.logical_value));
        volume_slide.negate();
    }

    if let Some(effect) = find_effect(cell, EffectType::VolumeIn) {
        *volume_slide = FixedPoint::from_digits(clamp_u16(effect.logical_value));
    }
}

fn build_line_speeds(
    speed_track: Option<&SpecialTrack>,
    height: usize,
    initial_speed: u8,
) -> (Vec<u8>, u8) {
    if height == 0 {
        return (Vec::new(), initial_speed.max(1));
    }

    let mut speeds = Vec::with_capacity(height);
    let mut current_speed = initial_speed.max(1);
    let mut cells: Vec<SpecialCell> = speed_track
        .map(|track| track.cells.clone())
        .unwrap_or_default();
    cells.sort_by_key(|cell| cell.index);
    let mut iter = cells.into_iter().peekable();

    for line in 0..height {
        while let Some(cell) = iter.peek() {
            if cell.index < line {
                iter.next();
            } else {
                break;
            }
        }

        if let Some(cell) = iter.peek()
            && cell.index == line
        {
            current_speed = sanitize_speed_value(cell.value, current_speed);
            iter.next();
        }

        speeds.push(current_speed);
    }

    (speeds, current_speed)
}

fn sanitize_speed_value(value: i32, fallback: u8) -> u8 {
    let clamped = value.clamp(1, 255) as u8;
    if clamped == 0 {
        fallback.max(1)
    } else {
        clamped
    }
}

fn sanitize_expression_index(value: i32, available: usize) -> usize {
    if available == 0 {
        return 0;
    }
    if value <= 0 {
        0
    } else {
        let index = value as usize;
        cmp::min(index, available)
    }
}

fn clamp_0_15(value: i32) -> u8 {
    value.clamp(0, 15) as u8
}

fn clamp_u16(value: i32) -> u16 {
    value.clamp(0, u16::MAX as i32) as u16
}

fn find_effect(cell: &Cell, effect_type: EffectType) -> Option<&CellEffect> {
    cell.effects
        .iter()
        .find(|effect| EffectType::from_name(&effect.name) == effect_type)
}

fn apply_inline_arpeggio(value: i32, is_four_notes: bool, line_context: &mut LineContext) {
    let byte = value.clamp(0, 0xFF) as u8;
    if byte == 0 {
        line_context.set_arpeggio_table(0);
        return;
    }

    let inline = if is_four_notes {
        InlineArpeggio::from_4_notes(byte)
    } else {
        InlineArpeggio::from_3_notes(byte)
    };

    line_context.set_inline_arpeggio(inline);
}
