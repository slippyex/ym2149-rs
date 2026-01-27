//! Arkos Tracker WASM player wrapper.
//!
//! Wraps `ArkosPlayer` to provide a consistent interface for the browser player.

use crate::YM_SAMPLE_RATE_F32;
use crate::metadata::YmMetadata;
use ym2149::Ym2149Backend;
use ym2149_arkos_replayer::ArkosPlayer;
use ym2149_common::{ChiptunePlayerBase, PlaybackState};

/// Arkos player wrapper for WebAssembly.
pub struct ArkosWasmPlayer {
    player: ArkosPlayer,
    estimated_frames: usize,
    duration_secs: f32,
}

impl ArkosWasmPlayer {
    /// Create a new Arkos WASM player wrapper.
    pub fn new(player: ArkosPlayer) -> (Self, YmMetadata) {
        let channel_count = player.channel_count();
        web_sys::console::log_1(&format!("ArkosWasmPlayer::new - channel_count: {channel_count}").into());

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
                duration_secs: duration_seconds,
            },
            metadata,
        )
    }

    /// Get duration in seconds.
    pub fn duration_seconds(&self) -> f32 {
        self.duration_secs
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
        self.player.current_tick_index()
    }

    /// Get total frame count.
    pub fn frame_count(&self) -> usize {
        self.estimated_frames
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

    /// Get number of channels (3 per PSG chip).
    pub fn channel_count(&self) -> usize {
        self.player.channel_count()
    }

    /// Dump registers for all PSG chips.
    pub fn dump_all_registers(&self) -> Vec<[u8; 16]> {
        let psg_count = self.player.channel_count().div_ceil(3);
        (0..psg_count)
            .filter_map(|i| self.player.chip(i).map(|c| c.dump_registers()))
            .collect()
    }

    /// Get current per-channel audio outputs for all PSG chips.
    ///
    /// Returns a vector of [A, B, C] arrays, one per PSG chip.
    pub fn get_channel_outputs(&self) -> Vec<[f32; 3]> {
        let psg_count = self.player.channel_count().div_ceil(3);
        (0..psg_count)
            .filter_map(|i| {
                self.player.chip(i).map(|c| {
                    let (a, b, c) = c.get_channel_outputs();
                    [a, b, c]
                })
            })
            .collect()
    }

    /// Generate samples with per-sample channel outputs for visualization.
    ///
    /// Fills the mono buffer with mixed samples and channels buffer with
    /// per-sample channel outputs for all PSG chips: [A0, B0, C0, A1, B1, C1, ...] per sample.
    pub fn generate_samples_with_channels_into(&mut self, mono: &mut [f32], channels: &mut [f32]) {
        let channel_count = self.player.channel_count();
        let psg_count = channel_count.div_ceil(3);

        // Generate samples one at a time to capture channel outputs
        let mut sample_buf = [0.0f32; 1];
        for (i, mono_sample) in mono.iter_mut().enumerate() {
            ChiptunePlayerBase::generate_samples_into(&mut self.player, &mut sample_buf);
            *mono_sample = sample_buf[0];
            let base = i * channel_count;
            for psg_idx in 0..psg_count {
                if let Some(chip) = self.player.chip(psg_idx) {
                    let (a, b, c) = chip.get_channel_outputs();
                    let offset = base + psg_idx * 3;
                    channels[offset] = a;
                    channels[offset + 1] = b;
                    channels[offset + 2] = c;
                }
            }
        }
    }
}
