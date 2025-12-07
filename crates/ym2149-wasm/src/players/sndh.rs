//! SNDH file WASM player wrapper.
//!
//! Wraps `SndhPlayer` to provide a consistent interface for the browser player.

use ym2149::Ym2149Backend;
use ym2149_common::{ChiptunePlayer, ChiptunePlayerBase, MetadataFields, PlaybackState};
use ym2149_sndh_replayer::{SndhPlayer, load_sndh};

use crate::YM_SAMPLE_RATE_F32;
use crate::metadata::YmMetadata;

/// SNDH player wrapper for WebAssembly.
pub struct SndhWasmPlayer {
    player: SndhPlayer,
}

impl SndhWasmPlayer {
    /// Create a new SNDH WASM player wrapper from raw data.
    pub fn new(data: &[u8]) -> Result<(Self, YmMetadata), String> {
        let sample_rate = YM_SAMPLE_RATE_F32 as u32;
        let mut player =
            load_sndh(data, sample_rate).map_err(|e| format!("Failed to load SNDH: {e}"))?;

        // Initialize default subsong
        let default_subsong = player.default_subsong();
        player
            .init_subsong(default_subsong)
            .map_err(|e| format!("Failed to init SNDH subsong: {e}"))?;

        let metadata = metadata_from_player(&player);
        Ok((Self { player }, metadata))
    }

    /// Start playback.
    pub fn play(&mut self) {
        ChiptunePlayerBase::play(&mut self.player);
    }

    /// Pause playback.
    pub fn pause(&mut self) {
        ChiptunePlayerBase::pause(&mut self.player);
    }

    /// Stop playback and reset.
    pub fn stop(&mut self) {
        ChiptunePlayerBase::stop(&mut self.player);
    }

    /// Get current playback state.
    pub fn state(&self) -> PlaybackState {
        ChiptunePlayerBase::state(&self.player)
    }

    /// Get current frame position.
    pub fn frame_position(&self) -> usize {
        0 // SNDH doesn't track frames like YM
    }

    /// Get total frame count.
    pub fn frame_count(&self) -> usize {
        0 // Unknown for SNDH
    }

    /// Get playback position as percentage (0.0 to 1.0).
    pub fn playback_position(&self) -> f32 {
        ChiptunePlayerBase::playback_position(&self.player)
    }

    /// Generate audio samples.
    pub fn generate_samples(&mut self, count: usize) -> Vec<f32> {
        ChiptunePlayerBase::generate_samples(&mut self.player, count)
    }

    /// Generate audio samples into a pre-allocated buffer.
    pub fn generate_samples_into(&mut self, buffer: &mut [f32]) {
        ChiptunePlayerBase::generate_samples_into(&mut self.player, buffer);
    }

    /// Mute or unmute a channel.
    pub fn set_channel_mute(&mut self, channel: usize, mute: bool) {
        ChiptunePlayerBase::set_channel_mute(&mut self.player, channel, mute);
    }

    /// Check if a channel is muted.
    pub fn is_channel_muted(&self, channel: usize) -> bool {
        ChiptunePlayerBase::is_channel_muted(&self.player, channel)
    }

    /// Dump current PSG register values.
    pub fn dump_registers(&self) -> [u8; 16] {
        self.player.ym2149().dump_registers()
    }

    /// Enable or disable the color filter.
    pub fn set_color_filter(&mut self, _enabled: bool) {
        // Not applicable for SNDH (uses real 68000 code)
    }

    /// Get number of subsongs.
    pub fn subsong_count(&self) -> usize {
        self.player.subsong_count()
    }

    /// Get current subsong (1-based).
    pub fn current_subsong(&self) -> usize {
        self.player.current_subsong()
    }

    /// Set subsong (1-based). Returns true on success.
    pub fn set_subsong(&mut self, index: usize) -> bool {
        self.player.init_subsong(index).is_ok()
    }
}

/// Convert SNDH player metadata to YmMetadata for WASM.
fn metadata_from_player(player: &SndhPlayer) -> YmMetadata {
    let meta = ChiptunePlayer::metadata(player);
    YmMetadata {
        title: if meta.title().is_empty() {
            "(unknown)".to_string()
        } else {
            meta.title().to_string()
        },
        author: if meta.author().is_empty() {
            "(unknown)".to_string()
        } else {
            meta.author().to_string()
        },
        comments: meta.comments().to_string(),
        format: "SNDH".to_string(),
        frame_count: 0,
        frame_rate: meta.frame_rate(),
        duration_seconds: 0.0, // Unknown for SNDH
    }
}
