//! AY file WASM player wrapper.
//!
//! Wraps `AyPlayer` to provide a consistent interface for the browser player.

use crate::metadata::{YmMetadata, metadata_from_ay};
use ym2149::Ym2149Backend;
use ym2149_ay_replayer::{
    AyMetadata as AyFileMetadata, AyPlayer, CPC_UNSUPPORTED_MSG, PlaybackState as AyState,
};
use ym2149_ym_replayer::PlaybackState;

/// AY player wrapper for WebAssembly.
pub struct AyWasmPlayer {
    player: AyPlayer,
    frame_count: usize,
    unsupported: bool,
}

impl AyWasmPlayer {
    /// Create a new AY WASM player wrapper.
    pub fn new(player: AyPlayer, meta: &AyFileMetadata) -> (Self, YmMetadata) {
        let metadata = metadata_from_ay(meta);
        let frame_count = metadata.frame_count as usize;
        (
            Self {
                player,
                frame_count,
                unsupported: false,
            },
            metadata,
        )
    }

    /// Start playback.
    pub fn play(&mut self) -> Result<(), String> {
        if self.unsupported {
            return Err(CPC_UNSUPPORTED_MSG.to_string());
        }
        self.player.play().map_err(|e| e.to_string())?;
        self.check_support()
    }

    /// Pause playback.
    pub fn pause(&mut self) -> Result<(), String> {
        self.player.pause();
        Ok(())
    }

    /// Stop playback and reset.
    pub fn stop(&mut self) -> Result<(), String> {
        self.player.stop().map_err(|e| e.to_string())
    }

    /// Get current playback state.
    pub fn state(&self) -> PlaybackState {
        match self.player.playback_state() {
            AyState::Playing => PlaybackState::Playing,
            AyState::Paused => PlaybackState::Paused,
            AyState::Stopped => PlaybackState::Stopped,
        }
    }

    /// Get current frame position.
    pub fn frame_position(&self) -> usize {
        self.player.current_frame()
    }

    /// Get total frame count.
    pub fn frame_count(&self) -> usize {
        self.frame_count
    }

    /// Get playback position as percentage (0.0 to 1.0).
    pub fn playback_position(&self) -> f32 {
        self.player.playback_position()
    }

    /// Generate audio samples.
    pub fn generate_samples(&mut self, count: usize) -> Vec<f32> {
        if self.unsupported {
            return vec![0.0; count];
        }
        let mut samples = self.player.generate_samples(count);
        if self.check_support().is_err() {
            samples.fill(0.0);
        }
        samples
    }

    /// Generate audio samples into a pre-allocated buffer.
    pub fn generate_samples_into(&mut self, buffer: &mut [f32]) {
        self.player.generate_samples_into(buffer);
        if self.check_support().is_err() {
            buffer.fill(0.0);
        }
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
        self.player.chip().dump_registers()
    }

    /// Enable or disable the color filter.
    pub fn set_color_filter(&mut self, enabled: bool) {
        self.player.set_color_filter(enabled);
    }

    fn check_support(&mut self) -> Result<(), String> {
        if self.unsupported || self.player.requires_cpc_firmware() {
            self.unsupported = true;
            Err(CPC_UNSUPPORTED_MSG.to_string())
        } else {
            Ok(())
        }
    }
}
