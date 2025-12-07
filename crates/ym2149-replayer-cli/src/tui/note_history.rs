//! Note history tracking for scrolling note display.
//!
//! Tracks the last N notes played on each channel for a scrolling
//! visualization in the Song Info panel.

use std::collections::VecDeque;

/// Number of notes to show (4 before + 1 current + 4 after = 9 visible)
pub const HISTORY_SIZE: usize = 9;

/// A single note entry with frequency and note name.
#[derive(Clone, Debug, Default)]
pub struct NoteEntry {
    /// Note name (e.g., "C4", "A#5", or "---" for silence)
    pub note: String,
    /// Frequency in Hz (0.0 for silence)
    pub freq: f32,
}

impl NoteEntry {
    /// Create a new note entry.
    pub fn new(note: String, freq: f32) -> Self {
        Self { note, freq }
    }

    /// Create a silence entry.
    pub fn silence() -> Self {
        Self {
            note: "---".to_string(),
            freq: 0.0,
        }
    }
}

/// Note history for a single channel.
#[derive(Clone, Debug)]
pub struct ChannelHistory {
    /// Ring buffer of notes (oldest first, newest last)
    notes: VecDeque<NoteEntry>,
    /// Current note index (the "active" one in the middle)
    current_idx: usize,
    /// Last frequency to detect note changes
    last_freq: f32,
    /// Last seen envelope shape (kept until a new one is set)
    last_envelope_shape: Option<String>,
}

impl Default for ChannelHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl ChannelHistory {
    /// Create a new channel history.
    pub fn new() -> Self {
        let mut notes = VecDeque::with_capacity(HISTORY_SIZE * 2);
        // Pre-fill with silence
        for _ in 0..HISTORY_SIZE {
            notes.push_back(NoteEntry::silence());
        }
        Self {
            notes,
            current_idx: HISTORY_SIZE / 2, // Middle position
            last_freq: 0.0,
            last_envelope_shape: None,
        }
    }

    /// Update with a new note. Only adds if frequency changed significantly.
    ///
    /// `has_output` should be true if the channel is producing sound (amplitude > 0 OR envelope_enabled).
    /// `envelope_shape` should be Some("shape") if envelope is enabled for this note.
    pub fn update(
        &mut self,
        note: &str,
        freq: f32,
        has_output: bool,
        envelope_shape: Option<&str>,
    ) {
        // Update last seen envelope shape if one is provided
        if let Some(shape) = envelope_shape {
            self.last_envelope_shape = Some(shape.to_string());
        }

        // Consider it a new note if:
        // 1. Frequency changed by more than 1%
        // 2. Or went from silence to sound (note on)
        let freq_changed = if self.last_freq > 0.0 && freq > 0.0 {
            ((freq - self.last_freq) / self.last_freq).abs() > 0.01
        } else {
            freq != self.last_freq
        };

        let is_note_on = has_output && freq > 0.0;

        if freq_changed && is_note_on {
            // Push new note
            self.notes.push_back(NoteEntry::new(note.to_string(), freq));

            // Keep buffer size reasonable
            while self.notes.len() > HISTORY_SIZE * 2 {
                self.notes.pop_front();
            }

            // Update current index to point to the new note
            self.current_idx = self.notes.len().saturating_sub(1);
        }

        self.last_freq = if is_note_on { freq } else { 0.0 };
    }

    /// Get the last seen envelope shape for this channel.
    pub fn last_envelope_shape(&self) -> Option<&str> {
        self.last_envelope_shape.as_deref()
    }

    /// Get visible notes (9 entries: 4 before, current, 4 after).
    /// Returns (notes, current_position) where current_position is 0-8.
    pub fn visible_notes(&self) -> (Vec<&NoteEntry>, usize) {
        let total = self.notes.len();
        if total == 0 {
            return (vec![], 0);
        }

        // We want to show: 4 before current, current, 4 after current
        // But "after" doesn't exist yet, so we show the last 9 with current at position 4
        let half = HISTORY_SIZE / 2; // 4

        // Calculate start index to get 9 notes with current at position 4
        let start = self.current_idx.saturating_sub(half);

        let end = (start + HISTORY_SIZE).min(total);
        let actual_start = if end - start < HISTORY_SIZE && end == total {
            total.saturating_sub(HISTORY_SIZE)
        } else {
            start
        };

        let visible: Vec<&NoteEntry> = self.notes.range(actual_start..end).collect();
        let current_pos = self.current_idx.saturating_sub(actual_start);
        let clamped_pos = current_pos.min(visible.len().saturating_sub(1));

        (visible, clamped_pos)
    }
}

/// Note history for all channels (up to 12 for 4 PSGs).
#[derive(Clone, Debug)]
pub struct NoteHistory {
    /// Per-channel history
    channels: [ChannelHistory; 12],
}

impl Default for NoteHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl NoteHistory {
    /// Create a new note history.
    pub fn new() -> Self {
        Self {
            channels: std::array::from_fn(|_| ChannelHistory::new()),
        }
    }

    /// Update a channel with new register data.
    ///
    /// `has_output` should be true if the channel is producing sound (amplitude > 0 OR envelope_enabled).
    /// `envelope_shape` should be Some("shape") if envelope is enabled for this note.
    pub fn update_channel(
        &mut self,
        channel: usize,
        note: &str,
        freq: f32,
        has_output: bool,
        envelope_shape: Option<&str>,
    ) {
        if channel < 12 {
            self.channels[channel].update(note, freq, has_output, envelope_shape);
        }
    }

    /// Get channel history.
    pub fn channel(&self, idx: usize) -> &ChannelHistory {
        &self.channels[idx.min(11)]
    }
}
