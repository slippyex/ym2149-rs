//! SNDH file WASM player wrapper.
//!
//! Wraps `SndhPlayer` to provide a consistent interface for the browser player.

use ym2149_common::{ChiptunePlayer, PlaybackMetadata, PlaybackState as SndhState};
use ym2149_sndh_replayer::{SndhPlayer, load_sndh};
use ym2149_ym_replayer::PlaybackState;

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
    pub fn play(&mut self) -> Result<(), String> {
        self.player.play();
        Ok(())
    }

    /// Pause playback.
    pub fn pause(&mut self) -> Result<(), String> {
        self.player.pause();
        Ok(())
    }

    /// Stop playback and reset.
    pub fn stop(&mut self) -> Result<(), String> {
        self.player.stop();
        Ok(())
    }

    /// Get current playback state.
    pub fn state(&self) -> PlaybackState {
        match self.player.state() {
            SndhState::Playing => PlaybackState::Playing,
            SndhState::Paused => PlaybackState::Paused,
            SndhState::Stopped => PlaybackState::Stopped,
        }
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
        0.0 // SNDH doesn't have precise position tracking
    }

    /// Generate audio samples.
    pub fn generate_samples(&mut self, count: usize) -> Vec<f32> {
        let mut buffer = vec![0.0; count];
        self.player.generate_samples_into(&mut buffer);
        buffer
    }

    /// Generate audio samples into a pre-allocated buffer.
    pub fn generate_samples_into(&mut self, buffer: &mut [f32]) {
        self.player.generate_samples_into(buffer);
    }

    /// Mute or unmute a channel.
    pub fn set_channel_mute(&mut self, _channel: usize, _mute: bool) {
        // TODO: Implement channel muting in SNDH player
    }

    /// Check if a channel is muted.
    pub fn is_channel_muted(&self, _channel: usize) -> bool {
        false
    }

    /// Dump current PSG register values.
    pub fn dump_registers(&self) -> [u8; 16] {
        self.player.ym2149().dump_registers()
    }

    /// Enable or disable the color filter.
    pub fn set_color_filter(&mut self, _enabled: bool) {
        // Not applicable for SNDH (uses real 68000 code)
    }
}

/// Convert SNDH player metadata to YmMetadata for WASM.
fn metadata_from_player(player: &SndhPlayer) -> YmMetadata {
    let meta = player.metadata();
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
