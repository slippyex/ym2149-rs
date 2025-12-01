//! Arkos Tracker WASM player wrapper.
//!
//! Wraps `ArkosPlayer` to provide a consistent interface for the browser player.

use crate::YM_SAMPLE_RATE_F32;
use crate::metadata::YmMetadata;
use ym2149::Ym2149Backend;
use ym2149_arkos_replayer::ArkosPlayer;
use ym2149_common::{ChiptunePlayer, PlaybackState};

/// Arkos player wrapper for WebAssembly.
pub struct ArkosWasmPlayer {
    player: ArkosPlayer,
    estimated_frames: usize,
}

impl ArkosWasmPlayer {
    /// Create a new Arkos WASM player wrapper.
    pub fn new(player: ArkosPlayer) -> (Self, YmMetadata) {
        let samples_per_frame = (YM_SAMPLE_RATE_F32 / player.replay_frequency_hz())
            .round()
            .max(1.0) as u32;
        let estimated_frames = player.estimated_total_ticks().max(1);
        let duration_seconds =
            (estimated_frames as f32 * samples_per_frame as f32) / YM_SAMPLE_RATE_F32;
        let info = player.metadata().clone();
        let frame_rate = player.replay_frequency_hz().round().max(1.0) as u32;

        let metadata = YmMetadata {
            title: info.title,
            author: if info.author.is_empty() {
                info.composer
            } else {
                info.author
            },
            comments: info.comments,
            format: "AKS".to_string(),
            frame_count: estimated_frames as u32,
            frame_rate,
            duration_seconds,
        };

        (
            Self {
                player,
                estimated_frames,
            },
            metadata,
        )
    }

    /// Start playback.
    pub fn play(&mut self) {
        ChiptunePlayer::play(&mut self.player);
    }

    /// Pause playback.
    pub fn pause(&mut self) {
        ChiptunePlayer::pause(&mut self.player);
    }

    /// Stop playback and reset.
    pub fn stop(&mut self) {
        ChiptunePlayer::stop(&mut self.player);
    }

    /// Get current playback state.
    pub fn state(&self) -> PlaybackState {
        ChiptunePlayer::state(&self.player)
    }

    /// Get current frame position.
    pub fn frame_position(&self) -> usize {
        self.player.current_tick_index()
    }

    /// Get total frame count.
    pub fn frame_count(&self) -> usize {
        self.estimated_frames
    }

    /// Get playback position as percentage (0.0 to 1.0).
    pub fn playback_position(&self) -> f32 {
        ChiptunePlayer::playback_position(&self.player)
    }

    /// Generate audio samples.
    pub fn generate_samples(&mut self, count: usize) -> Vec<f32> {
        ChiptunePlayer::generate_samples(&mut self.player, count)
    }

    /// Generate audio samples into a pre-allocated buffer.
    pub fn generate_samples_into(&mut self, buffer: &mut [f32]) {
        ChiptunePlayer::generate_samples_into(&mut self.player, buffer);
    }

    /// Mute or unmute a channel.
    pub fn set_channel_mute(&mut self, channel: usize, mute: bool) {
        ChiptunePlayer::set_channel_mute(&mut self.player, channel, mute);
    }

    /// Check if a channel is muted.
    pub fn is_channel_muted(&self, channel: usize) -> bool {
        ChiptunePlayer::is_channel_muted(&self.player, channel)
    }

    /// Dump current PSG register values.
    pub fn dump_registers(&self) -> [u8; 16] {
        self.player
            .chip(0)
            .map(|chip| chip.dump_registers())
            .unwrap_or([0; 16])
    }

    /// Enable or disable the color filter.
    pub fn set_color_filter(&mut self, enabled: bool) {
        if let Some(chip) = self.player.chip_mut(0) {
            chip.set_color_filter(enabled);
        }
    }
}
