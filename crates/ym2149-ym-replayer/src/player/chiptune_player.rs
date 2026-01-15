//! ChiptunePlayer trait implementation for YM6 player.
//!
//! This module implements the unified `ChiptunePlayer` trait from `ym2149-common`,
//! providing a common interface for YM file playback alongside other chiptune formats.

use super::PlaybackState;
use super::ym_player::YmPlayerGeneric;
use super::ym6::Ym6Info;
use ym2149::Ym2149Backend;
use ym2149_common::{ChiptunePlayer, ChiptunePlayerBase, MetadataFields};

/// Metadata wrapper for YM6 files.
///
/// This struct combines optional `Ym6Info` with fallback values
/// to ensure metadata is always available via the `ChiptunePlayer` trait.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ym6Metadata {
    /// Song title
    pub title: String,
    /// Author/composer name
    pub author: String,
    /// Additional comments
    pub comments: String,
    /// Total frame count
    pub frame_count: usize,
    /// Playback frame rate in Hz
    pub frame_rate: u32,
    /// Loop start frame
    pub loop_frame: Option<usize>,
}

impl Ym6Metadata {
    /// Create metadata from Ym6Info.
    pub fn from_info(info: &Ym6Info, loop_frame: Option<usize>) -> Self {
        Self {
            title: info.song_name.clone(),
            author: info.author.clone(),
            comments: info.comment.clone(),
            frame_count: info.frame_count as usize,
            frame_rate: info.frame_rate as u32,
            loop_frame,
        }
    }

    /// Create default metadata for manually loaded frames.
    pub fn from_frames(frame_count: usize, loop_frame: Option<usize>) -> Self {
        Self {
            title: String::new(),
            author: String::new(),
            comments: String::new(),
            frame_count,
            frame_rate: 50,
            loop_frame,
        }
    }
}

impl MetadataFields for Ym6Metadata {
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
        "YM"
    }

    fn frame_count(&self) -> Option<usize> {
        Some(self.frame_count)
    }

    fn frame_rate(&self) -> u32 {
        self.frame_rate
    }

    fn loop_frame(&self) -> Option<usize> {
        self.loop_frame
    }
}

impl<B: Ym2149Backend> ChiptunePlayerBase for YmPlayerGeneric<B> {
    fn play(&mut self) {
        let _ = <Self as super::PlaybackController>::play(self);
    }

    fn pause(&mut self) {
        let _ = <Self as super::PlaybackController>::pause(self);
    }

    fn stop(&mut self) {
        let _ = <Self as super::PlaybackController>::stop(self);
    }

    fn state(&self) -> PlaybackState {
        <Self as super::PlaybackController>::state(self)
    }

    fn generate_samples_into(&mut self, buffer: &mut [f32]) {
        YmPlayerGeneric::generate_samples_into(self, buffer);
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn playback_position(&self) -> f32 {
        YmPlayerGeneric::playback_position(self)
    }

    fn set_channel_mute(&mut self, channel: usize, mute: bool) {
        YmPlayerGeneric::set_channel_mute(self, channel, mute);
    }

    fn is_channel_muted(&self, channel: usize) -> bool {
        YmPlayerGeneric::is_channel_muted(self, channel)
    }

    fn seek(&mut self, position: f32) -> bool {
        let frame_count = self.frame_count();
        if frame_count == 0 {
            return false;
        }
        let target_frame = (position.clamp(0.0, 1.0) * frame_count as f32) as usize;
        self.seek_frame(target_frame);
        true
    }

    fn duration_seconds(&self) -> f32 {
        let frame_count = self.frame_count();
        let samples_per_frame = self.samples_per_frame_value() as f32;
        let sample_rate = self.sample_rate() as f32;
        (frame_count as f32 * samples_per_frame) / sample_rate
    }
}

impl<B: Ym2149Backend> ChiptunePlayer for YmPlayerGeneric<B> {
    type Metadata = Ym6Metadata;

    fn metadata(&self) -> &Self::Metadata {
        &self.cached_metadata
    }
}
