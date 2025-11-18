//! Expression system for arpeggios and pitch tables
//!
//! Expressions in Arkos are sequences of values that can be applied to notes
//! (arpeggios) or periods (pitch tables). They have speed, looping, and can
//! be referenced by index or used inline from effects.

use crate::format::AksSong;

/// An inline arpeggio expression (from 3-note or 4-note effects)
///
/// This is built on-the-fly from effect values and doesn't reference
/// the song's arpeggio table.
#[derive(Debug, Clone, PartialEq)]
pub struct InlineArpeggio {
    values: Vec<i8>,
    speed: u8,
    loop_start: usize,
    end: usize,
}

impl InlineArpeggio {
    /// Create empty inline arpeggio
    pub fn empty() -> Self {
        Self {
            values: vec![0],
            speed: 0,
            loop_start: 0,
            end: 0,
        }
    }

    /// Build a 3-note arpeggio from effect value
    ///
    /// Effect format: 0xXY where X and Y are note offsets
    /// Creates arpeggio: [0, X, Y] with speed 0 (every tick)
    pub fn from_3_notes(effect_value: u8) -> Self {
        let note1 = ((effect_value >> 4) & 0x0F) as i8;
        let note2 = (effect_value & 0x0F) as i8;

        Self {
            values: vec![0, note1, note2],
            speed: 0,
            loop_start: 0,
            end: 2,
        }
    }

    /// Build a 4-note arpeggio from effect value
    ///
    /// Effect format: 0xXY where X and Y are note offsets
    /// Creates arpeggio: [0, X, Y, X+Y] with speed 0 (every tick)
    pub fn from_4_notes(effect_value: u8) -> Self {
        let note1 = ((effect_value >> 4) & 0x0F) as i8;
        let note2 = (effect_value & 0x0F) as i8;
        let note3 = note1 + note2;

        Self {
            values: vec![0, note1, note2, note3],
            speed: 0,
            loop_start: 0,
            end: 3,
        }
    }

    /// Get value at index
    pub fn get(&self, index: usize) -> i8 {
        if index < self.values.len() {
            self.values[index]
        } else {
            0
        }
    }

    /// Get speed
    pub fn speed(&self) -> u8 {
        self.speed
    }

    /// Get loop start index
    pub fn loop_start(&self) -> usize {
        self.loop_start
    }

    /// Get end index
    pub fn end(&self) -> usize {
        self.end
    }

    /// Get length
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Check if empty (only default value)
    pub fn is_empty(&self) -> bool {
        self.values.len() <= 1
    }

    /// Clear to empty state
    pub fn clear(&mut self) {
        self.values = vec![0];
        self.speed = 0;
        self.loop_start = 0;
        self.end = 0;
    }

    /// Set speed
    pub fn set_speed(&mut self, speed: u8) {
        self.speed = speed;
    }
}

/// State for reading an expression (arpeggio or pitch table)
#[derive(Debug, Clone)]
pub struct ExpressionReader {
    current_index: usize,
    current_tick: u8,
    speed: u8,
    loop_start: usize,
    /// End index (public for boundary checks)
    pub end: usize,
    forced_speed: Option<u8>,
}

impl ExpressionReader {
    /// Create new reader at start
    pub fn new() -> Self {
        Self {
            current_index: 0,
            current_tick: 0,
            speed: 0,
            loop_start: 0,
            end: 0,
            forced_speed: None,
        }
    }

    /// Reset to beginning
    pub fn reset(&mut self) {
        self.current_index = 0;
        self.current_tick = 0;
    }

    /// Update metadata from song or inline arpeggio
    pub fn update_metadata(&mut self, speed: u8, loop_start: usize, end: usize) {
        self.speed = speed;
        self.loop_start = loop_start;
        self.end = end;
    }

    /// Set forced speed (from effect)
    pub fn set_forced_speed(&mut self, speed: Option<u8>) {
        self.forced_speed = speed;
    }

    /// Get effective speed (forced or normal)
    pub fn effective_speed(&self) -> u8 {
        self.forced_speed.unwrap_or(self.speed)
    }

    /// Advance to next value if needed
    ///
    /// Returns true if we moved to next index
    pub fn advance(&mut self) -> bool {
        let speed = self.effective_speed();

        self.current_tick += 1;
        if self.current_tick > speed {
            self.current_tick = 0;
            self.current_index += 1;

            // Check for loop
            if self.current_index > self.end {
                self.current_index = self.loop_start;
            }

            // Clamp to end (safety)
            if self.current_index > self.end {
                self.current_index = 0;
            }

            return true;
        }

        false
    }

    /// Get current index
    pub fn current_index(&self) -> usize {
        self.current_index
    }

    /// Set current index (for safety corrections)
    pub fn set_current_index(&mut self, index: usize) {
        self.current_index = index;
    }
}

impl Default for ExpressionReader {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper to read arpeggio values from song
pub fn read_arpeggio_value(song: &AksSong, arpeggio_index: usize, value_index: usize) -> i8 {
    if arpeggio_index >= song.arpeggios.len() {
        return 0;
    }

    let arpeggio = &song.arpeggios[arpeggio_index];
    if value_index < arpeggio.shift {
        return 0;
    }
    let relative_index = value_index - arpeggio.shift;

    if relative_index < arpeggio.values.len() {
        arpeggio.values[relative_index]
    } else {
        0
    }
}

/// Get arpeggio metadata from song
pub fn get_arpeggio_metadata(song: &AksSong, arpeggio_index: usize) -> (u8, usize, usize) {
    if arpeggio_index >= song.arpeggios.len() {
        return (0, 0, 0);
    }

    let arpeggio = &song.arpeggios[arpeggio_index];
    (
        arpeggio.speed,
        arpeggio.loop_start + arpeggio.shift,
        arpeggio.end_index + arpeggio.shift,
    )
}

/// Helper to read pitch table values from song
pub fn read_pitch_value(song: &AksSong, pitch_index: usize, value_index: usize) -> i16 {
    if pitch_index >= song.pitch_tables.len() {
        return 0;
    }

    let pitch_table = &song.pitch_tables[pitch_index];
    if value_index < pitch_table.shift {
        return 0;
    }
    let relative_index = value_index - pitch_table.shift;

    if relative_index < pitch_table.values.len() {
        pitch_table.values[relative_index]
    } else {
        0
    }
}

/// Get pitch table metadata from song
pub fn get_pitch_metadata(song: &AksSong, pitch_index: usize) -> (u8, usize, usize) {
    if pitch_index >= song.pitch_tables.len() {
        return (0, 0, 0);
    }

    let pitch_table = &song.pitch_tables[pitch_index];
    (
        pitch_table.speed,
        pitch_table.loop_start + pitch_table.shift,
        pitch_table.end_index + pitch_table.shift,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inline_arpeggio_3_notes() {
        let arp = InlineArpeggio::from_3_notes(0x37); // 0, 3, 7
        assert_eq!(arp.len(), 3);
        assert_eq!(arp.get(0), 0);
        assert_eq!(arp.get(1), 3);
        assert_eq!(arp.get(2), 7);
        assert_eq!(arp.speed(), 0);
    }

    #[test]
    fn test_inline_arpeggio_4_notes() {
        let arp = InlineArpeggio::from_4_notes(0x37); // 0, 3, 7, 10
        assert_eq!(arp.len(), 4);
        assert_eq!(arp.get(0), 0);
        assert_eq!(arp.get(1), 3);
        assert_eq!(arp.get(2), 7);
        assert_eq!(arp.get(3), 10);
    }

    #[test]
    fn test_expression_reader_advance() {
        let mut reader = ExpressionReader::new();
        reader.update_metadata(1, 0, 2); // speed=1, loop 0-2

        assert_eq!(reader.current_index(), 0);
        reader.advance();
        assert_eq!(reader.current_index(), 0); // Still at 0 (tick 1)
        reader.advance();
        assert_eq!(reader.current_index(), 1); // Moved to 1 (tick > speed)
        reader.advance();
        reader.advance();
        assert_eq!(reader.current_index(), 2);
        reader.advance();
        reader.advance();
        assert_eq!(reader.current_index(), 0); // Looped back
    }
}
