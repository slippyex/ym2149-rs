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
        self.player.current_frame() as usize
    }

    /// Get total frame count.
    ///
    /// Returns 0 if duration is unknown (from FRMS tag or TIME fallback).
    pub fn frame_count(&self) -> usize {
        self.player.total_frames() as usize
    }

    /// Get playback position as percentage (0.0 to 1.0).
    pub fn playback_position(&self) -> f32 {
        self.player.progress()
    }

    /// Seek to a specific frame.
    ///
    /// Returns true on success. Seeking re-initializes and fast-forwards.
    pub fn seek_frame(&mut self, frame: usize) -> bool {
        self.player.seek_to_frame(frame as u32).is_ok()
    }

    /// Seek to a percentage position (0.0 to 1.0).
    ///
    /// Returns true on success. Works for all SNDH files (uses fallback duration for older files).
    pub fn seek_percentage(&mut self, position: f32) -> bool {
        ChiptunePlayerBase::seek(&mut self.player, position)
    }

    /// Get duration in seconds.
    ///
    /// For SNDH < 2.2 without FRMS/TIME, returns 300 (5 minute fallback).
    pub fn duration_seconds(&self) -> f32 {
        ChiptunePlayerBase::duration_seconds(&self.player)
    }

    /// Check if the duration is from actual metadata (FRMS/TIME) or estimated.
    ///
    /// Returns false for older SNDH files using the 5-minute fallback.
    pub fn has_duration_info(&self) -> bool {
        self.player.has_duration_info()
    }

    /// Generate mono audio samples.
    pub fn generate_samples(&mut self, count: usize) -> Vec<f32> {
        ChiptunePlayerBase::generate_samples(&mut self.player, count)
    }

    /// Generate mono audio samples into a pre-allocated buffer.
    pub fn generate_samples_into(&mut self, buffer: &mut [f32]) {
        ChiptunePlayerBase::generate_samples_into(&mut self.player, buffer);
    }

    /// Generate stereo audio samples (interleaved L/R).
    ///
    /// Returns stereo samples with STE DAC and LMC1992 audio processing.
    pub fn generate_samples_stereo(&mut self, frame_count: usize) -> Vec<f32> {
        let mut buffer = vec![0.0f32; frame_count * 2];
        self.player.render_f32_stereo(&mut buffer);
        buffer
    }

    /// Generate stereo audio samples into a pre-allocated buffer (interleaved L/R).
    ///
    /// Buffer length must be even (frame_count * 2).
    pub fn generate_samples_into_stereo(&mut self, buffer: &mut [f32]) {
        self.player.render_f32_stereo(buffer);
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
    let frame_count = player.total_frames();
    let frame_rate = meta.frame_rate();
    let duration_seconds = if frame_count > 0 && frame_rate > 0 {
        frame_count as f32 / frame_rate as f32
    } else {
        0.0
    };

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
        frame_count,
        frame_rate,
        duration_seconds,
    }
}
