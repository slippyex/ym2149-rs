//! ChiptunePlayer trait implementation for Arkos player.
//!
//! This module implements the unified `ChiptunePlayer` trait from `ym2149-common`,
//! providing a common interface for AKS file playback alongside other chiptune formats.

use super::ArkosPlayer;
use ym2149_common::{ChiptunePlayer, PlaybackMetadata, PlaybackState};

/// Metadata wrapper for Arkos songs.
///
/// This struct wraps `SongMetadata` and provides additional computed fields
/// needed for the `ChiptunePlayer` interface.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ArkosMetadata {
    /// Song title
    pub title: String,
    /// Author/composer name
    pub author: String,
    /// Additional comments
    pub comments: String,
    /// Estimated total lines (tick count / speed)
    pub estimated_lines: usize,
    /// Replay frequency in Hz
    pub replay_frequency: f32,
}

impl ArkosMetadata {
    /// Create metadata from ArkosPlayer state.
    pub fn from_player(player: &ArkosPlayer) -> Self {
        let song_meta = &player.song.metadata;
        let subsong = &player.song.subsongs[player.subsong_index];

        // Calculate estimated total lines
        let estimated_lines: usize = subsong.positions.iter().map(|pos| pos.height).sum();

        Self {
            title: song_meta.title.clone(),
            author: if song_meta.author.is_empty() {
                song_meta.composer.clone()
            } else {
                song_meta.author.clone()
            },
            comments: song_meta.comments.clone(),
            estimated_lines,
            replay_frequency: subsong.replay_frequency_hz,
        }
    }
}

impl PlaybackMetadata for ArkosMetadata {
    fn title(&self) -> &str {
        &self.title
    }

    fn author(&self) -> &str {
        &self.author
    }

    fn comments(&self) -> &str {
        &self.comments
    }

    fn format(&self) -> &str {
        "AKS"
    }

    fn frame_count(&self) -> Option<usize> {
        Some(self.estimated_lines)
    }

    fn frame_rate(&self) -> u32 {
        self.replay_frequency as u32
    }
}

impl ChiptunePlayer for ArkosPlayer {
    type Metadata = ArkosMetadata;

    fn play(&mut self) {
        let _ = ArkosPlayer::play(self);
    }

    fn pause(&mut self) {
        let _ = ArkosPlayer::pause(self);
    }

    fn stop(&mut self) {
        let _ = ArkosPlayer::stop(self);
    }

    fn state(&self) -> PlaybackState {
        if self.is_playing() {
            PlaybackState::Playing
        } else {
            PlaybackState::Stopped
        }
    }

    fn metadata(&self) -> &Self::Metadata {
        &self.cached_metadata
    }

    fn generate_samples_into(&mut self, buffer: &mut [f32]) {
        ArkosPlayer::generate_samples_into(self, buffer);
    }

    fn sample_rate(&self) -> u32 {
        self.output_sample_rate() as u32
    }
}
