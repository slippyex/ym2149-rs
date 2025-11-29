//! Arkos Tracker WASM player wrapper.
//!
//! Wraps `ArkosPlayer` to provide a consistent interface for the browser player.

use crate::YM_SAMPLE_RATE_F32;
use crate::metadata::YmMetadata;
use ym2149::Ym2149Backend;
use ym2149_arkos_replayer::ArkosPlayer;
use ym2149_ym_replayer::PlaybackState;

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
    pub fn play(&mut self) -> Result<(), String> {
        self.player
            .play()
            .map_err(|e| format!("AKS play failed: {e}"))
    }

    /// Pause playback.
    pub fn pause(&mut self) -> Result<(), String> {
        self.player
            .pause()
            .map_err(|e| format!("AKS pause failed: {e}"))
    }

    /// Stop playback and reset.
    pub fn stop(&mut self) -> Result<(), String> {
        self.player
            .stop()
            .map_err(|e| format!("AKS stop failed: {e}"))
    }

    /// Get current playback state.
    pub fn state(&self) -> PlaybackState {
        if self.player.is_playing() {
            PlaybackState::Playing
        } else {
            PlaybackState::Paused
        }
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
        if self.estimated_frames == 0 {
            0.0
        } else {
            self.player.current_tick_index() as f32 / self.estimated_frames as f32
        }
    }

    /// Generate audio samples.
    pub fn generate_samples(&mut self, count: usize) -> Vec<f32> {
        self.player.generate_samples(count)
    }

    /// Generate audio samples into a pre-allocated buffer.
    pub fn generate_samples_into(&mut self, buffer: &mut [f32]) {
        self.player.generate_samples_into(buffer);
    }

    /// Mute or unmute a channel.
    pub fn set_channel_mute(&mut self, channel: usize, mute: bool) {
        self.player.set_channel_mute(channel, mute);
    }

    /// Check if a channel is muted.
    pub fn is_channel_muted(&self, channel: usize) -> bool {
        self.player.is_channel_muted(channel)
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
