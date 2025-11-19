//! PSG helper functions (period calculations, note clamping)
#![allow(missing_docs)]

use crate::format::Note;

const START_OCTAVE: i32 = -3;
const NOTES_IN_OCTAVE: i32 = 12;

/// Clamp a note index to Arkos Tracker's valid range (0-127, 255 = off)
pub fn clamp_note(note: i32) -> Note {
    note.clamp(0, 127) as Note
}

/// Calculate PSG period for a given note using Arkos Tracker's formula
///
/// Mirrors `arkostracker3/source/player/PsgPeriod.cpp`.
/// The resulting period is clamped to 0..0xFFFF, matching the reference.
pub fn calculate_period(psg_frequency: f64, reference_frequency: f64, note: Note) -> u16 {
    if note == 255 {
        return 0;
    }

    let octave = (note as i32 / NOTES_IN_OCTAVE) + START_OCTAVE;
    let note_in_octave = (note as i32 % NOTES_IN_OCTAVE) + 1;

    let frequency =
        reference_frequency * 2f64.powf((octave as f64) + ((note_in_octave as f64 - 10.0) / 12.0));
    let period_divider = psg_frequency / 8.0;
    let period = (period_divider / frequency).round();

    period.clamp(0.0, 65535.0) as u16
}

/// Precomputed period table and helpers for reverse conversions
/// Lookup table providing both note→period and period→note mappings
use std::collections::BTreeMap;

#[derive(Clone)]
pub struct PsgPeriodTable {
    note_to_period: Vec<u16>,
    period_entries: Vec<(u16, i32)>,
}

impl PsgPeriodTable {
    /// Build a period table for the supplied PSG/reference frequencies
    pub fn new(psg_frequency: f64, reference_frequency: f64) -> Self {
        let mut note_to_period = Vec::with_capacity(128);
        let mut period_to_note = BTreeMap::new();
        for note in 0..=127 {
            let period = calculate_period(psg_frequency, reference_frequency, note as Note);
            note_to_period.push(period);
            period_to_note.entry(period).or_insert(note);
        }
        let period_entries = period_to_note.into_iter().collect();
        Self {
            note_to_period,
            period_entries,
        }
    }

    /// Mimic AT3's `PsgPeriod::findNoteAndShift` (lower_bound + closest match)
    pub fn find_note_and_shift(&self, period: i32) -> AbsoluteNote {
        let period = period.clamp(0, i32::MAX) as u16;
        if self.period_entries.is_empty() {
            return AbsoluteNote {
                note_index: 0,
                shift: period as i32,
            };
        }

        let mut idx = 0;
        while idx < self.period_entries.len() && self.period_entries[idx].0 < period {
            idx += 1;
        }
        if idx >= self.period_entries.len() {
            idx = self.period_entries.len() - 1;
        }
        let (found_period, mut found_note) = (
            self.period_entries[idx].0 as i32,
            self.period_entries[idx].1,
        );

        if found_period == period as i32 {
            return AbsoluteNote {
                note_index: found_note,
                shift: 0,
            };
        }

        let mut found_shift = found_period - period as i32;

        if idx > 0 {
            let prev_period = self.period_entries[idx - 1].0 as i32;
            let new_shift = period as i32 - prev_period;
            if new_shift.abs() < found_shift.abs() {
                found_note += 1;
                found_shift = -new_shift;
            }
        }

        AbsoluteNote {
            note_index: found_note,
            shift: found_shift,
        }
    }

    /// Return the direct period for a note index
    pub fn period_for_note(&self, note_index: usize) -> u16 {
        self.note_to_period.get(note_index).copied().unwrap_or(0)
    }
}

/// Absolute note index + detune shift (in periods)
#[derive(Clone, Copy)]
pub struct AbsoluteNote {
    /// Note index relative to Arkos scale (0-127, may exceed in calculations)
    pub note_index: i32,
    /// Signed period offset relative to the base note
    pub shift: i32,
}

impl AbsoluteNote {
    /// Convert to note-in-octave + octave pair (handles negatives)
    pub fn note_in_octave_and_octave(self) -> (i32, i32) {
        split_note(self.note_index)
    }
}

/// Split a relative note index into (note_in_octave, octave)
pub fn split_note(note: i32) -> (i32, i32) {
    let octave = note.div_euclid(NOTES_IN_OCTAVE);
    let note_in_octave = note.rem_euclid(NOTES_IN_OCTAVE);
    (note_in_octave, octave)
}
