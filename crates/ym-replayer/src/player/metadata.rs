//! Metadata and Information Management
//!
//! This module handles song metadata access, formatting, and active effect queries.

use super::ym_player::Ym6PlayerGeneric;
use super::ym6::Ym6Info;
use ym2149::Ym2149Backend;

impl<B: Ym2149Backend> Ym6PlayerGeneric<B> {
    /// Get song metadata if available
    pub fn info(&self) -> Option<&Ym6Info> {
        self.info.as_ref()
    }

    /// Set song metadata
    pub fn set_info(&mut self, info: Ym6Info) {
        self.info = Some(info);
    }

    /// Clone register frames (for non-tracker modes)
    #[allow(missing_docs)]
    pub fn frames_clone(&self) -> Option<Vec<[u8; 16]>> {
        if self.is_tracker_mode {
            None
        } else {
            Some(self.sequencer.frames().to_vec())
        }
    }

    /// Check if player is in tracker mode
    #[allow(missing_docs)]
    pub fn is_tracker_mode(&self) -> bool {
        self.is_tracker_mode
    }

    /// Get current active effects status for visualization
    ///
    /// Returns tuple of (sync_buzzer_active, sid_active_per_voice, drum_active_per_voice)
    pub fn get_active_effects(&self) -> (bool, [bool; 3], [bool; 3]) {
        self.effects.effect_flags()
    }

    /// Format playback information as human-readable string
    ///
    /// # Returns
    /// A formatted string containing song metadata (if available) and playback info
    ///
    /// # Example
    /// ```ignore
    /// println!("File Information:");
    /// println!("{}", player.format_info());
    /// ```
    pub fn format_info(&self) -> String {
        let duration = self.get_duration_seconds();
        let frame_count = self.frame_count();

        if let Some(info) = self.info() {
            format!(
                "  Song: {}\n  Author: {}\n  Comment: {}\n  Duration: {:.2}s ({} frames @ {}Hz)\n  Master Clock: {} Hz",
                info.song_name,
                info.author,
                info.comment,
                duration,
                frame_count,
                info.frame_rate,
                info.master_clock
            )
        } else {
            format!(
                "  Duration: {:.2}s ({} frames @ 50Hz)\n  Master Clock: 2,000,000 Hz",
                duration, frame_count
            )
        }
    }
}
