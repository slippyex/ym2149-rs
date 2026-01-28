//! AY file WASM player wrapper.
//!
//! Wraps `AyPlayer` to provide a consistent interface for the browser player.

use crate::metadata::{YmMetadata, metadata_from_ay};
use ym2149::Ym2149Backend;
use ym2149_ay_replayer::{AyMetadata as AyFileMetadata, AyPlayer, CPC_UNSUPPORTED_MSG};
use ym2149_common::{ChiptunePlayerBase, PlaybackState};

/// AY player wrapper for WebAssembly.
pub struct AyWasmPlayer {
    player: AyPlayer,
    frame_count: usize,
    duration_secs: f32,
    unsupported: bool,
}

impl AyWasmPlayer {
    /// Create a new AY WASM player wrapper.
    pub fn new(player: AyPlayer, meta: &AyFileMetadata) -> (Self, YmMetadata) {
        let metadata = metadata_from_ay(meta);
        let frame_count = metadata.frame_count as usize;
        let duration_secs = metadata.duration_seconds;
        (
            Self {
                player,
                frame_count,
                duration_secs,
                unsupported: false,
            },
            metadata,
        )
    }

    /// Get duration in seconds.
    pub fn duration_seconds(&self) -> f32 {
        self.duration_secs
    }

    /// Start playback.
    pub fn play(&mut self) -> Result<(), String> {
        if self.unsupported {
            return Err(CPC_UNSUPPORTED_MSG.to_string());
        }
        ChiptunePlayerBase::play(&mut self.player);
        self.check_support()
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
        self.player.current_frame()
    }

    /// Get total frame count.
    pub fn frame_count(&self) -> usize {
        self.frame_count
    }

    /// Get playback position as percentage (0.0 to 1.0).
    pub fn playback_position(&self) -> f32 {
        ChiptunePlayerBase::playback_position(&self.player)
    }

    /// Generate audio samples.
    pub fn generate_samples(&mut self, count: usize) -> Vec<f32> {
        if self.unsupported {
            return vec![0.0; count];
        }
        let mut samples = ChiptunePlayerBase::generate_samples(&mut self.player, count);
        if self.check_support().is_err() {
            samples.fill(0.0);
        }
        samples
    }

    /// Generate audio samples into a pre-allocated buffer.
    pub fn generate_samples_into(&mut self, buffer: &mut [f32]) {
        ChiptunePlayerBase::generate_samples_into(&mut self.player, buffer);
        if self.check_support().is_err() {
            buffer.fill(0.0);
        }
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
        self.player.chip().dump_registers()
    }

    /// Get current per-channel audio outputs.
    ///
    /// Returns the actual audio output values (A, B, C) updated at sample rate.
    pub fn get_channel_outputs(&self) -> (f32, f32, f32) {
        self.player.chip().get_channel_outputs()
    }

    /// Enable or disable the color filter.
    pub fn set_color_filter(&mut self, enabled: bool) {
        self.player.set_color_filter(enabled);
    }

    /// Generate samples with per-sample channel outputs for visualization.
    ///
    /// Fills the mono buffer with mixed samples and channels buffer with
    /// per-sample channel outputs: [A, B, C, A, B, C, ...].
    ///
    /// Note: AY player generates samples in frame-sized batches internally,
    /// so channel outputs are captured after each sample but may reflect
    /// the frame-end state for cached samples.
    pub fn generate_samples_with_channels_into(&mut self, mono: &mut [f32], channels: &mut [f32]) {
        if self.unsupported {
            mono.fill(0.0);
            channels.fill(0.0);
            return;
        }

        // Generate samples one at a time to capture channel outputs
        let mut sample_buf = [0.0f32; 1];
        for i in 0..mono.len() {
            self.player.generate_samples_into(&mut sample_buf);
            mono[i] = sample_buf[0];
            let (a, b, c) = self.player.chip().get_channel_outputs();
            channels[i * 3] = a;
            channels[i * 3 + 1] = b;
            channels[i * 3 + 2] = c;
        }

        if self.check_support().is_err() {
            mono.fill(0.0);
            channels.fill(0.0);
        }
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
