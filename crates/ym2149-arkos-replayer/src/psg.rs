//! PSG helper functions (period calculations, note clamping)

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

/// Split a relative note index into (note_in_octave, octave)
pub fn split_note(note: i32) -> (i32, i32) {
    let octave = note.div_euclid(NOTES_IN_OCTAVE);
    let note_in_octave = note.rem_euclid(NOTES_IN_OCTAVE);
    (note_in_octave, octave)
}
