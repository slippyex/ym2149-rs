//! Player wrapper types for different file formats.
//!
//! This module provides unified access to YM, Arkos, AY, and SNDH players
//! through the `BrowserSongPlayer` enum.

pub mod arkos;
pub mod ay;
pub mod sndh;

use arkos::ArkosWasmPlayer;
use ay::AyWasmPlayer;
use sndh::SndhWasmPlayer;
use ym2149::Ym2149Backend;
use ym2149_common::{ChiptunePlayerBase, PlaybackState};

/// Unified player enum for all supported formats.
pub enum BrowserSongPlayer {
    /// YM format player (YM2-YM6).
    Ym(Box<ym2149_ym_replayer::YmPlayer>),
    /// Arkos Tracker format player (.aks).
    Arkos(Box<ArkosWasmPlayer>),
    /// AY format player (.ay).
    Ay(Box<AyWasmPlayer>),
    /// SNDH format player (Atari ST).
    Sndh(Box<SndhWasmPlayer>),
}

impl BrowserSongPlayer {
    /// Seek to a specific frame.
    ///
    /// Returns `true` if seek is supported and successful, `false` otherwise.
    pub fn seek_frame(&mut self, frame: usize) -> bool {
        match self {
            BrowserSongPlayer::Ym(player) => {
                player.seek_frame(frame);
                true
            }
            BrowserSongPlayer::Arkos(_) => false,
            BrowserSongPlayer::Ay(_) => false,
            BrowserSongPlayer::Sndh(_) => false,
        }
    }

    /// Start playback.
    pub fn play(&mut self) {
        match self {
            BrowserSongPlayer::Ym(player) => player.play(),
            BrowserSongPlayer::Arkos(player) => player.play(),
            BrowserSongPlayer::Ay(player) => {
                let _ = player.play();
            }
            BrowserSongPlayer::Sndh(player) => player.play(),
        }
    }

    /// Pause playback.
    pub fn pause(&mut self) {
        match self {
            BrowserSongPlayer::Ym(player) => player.pause(),
            BrowserSongPlayer::Arkos(player) => player.pause(),
            BrowserSongPlayer::Ay(player) => player.pause(),
            BrowserSongPlayer::Sndh(player) => player.pause(),
        }
    }

    /// Stop playback and reset.
    pub fn stop(&mut self) {
        match self {
            BrowserSongPlayer::Ym(player) => player.stop(),
            BrowserSongPlayer::Arkos(player) => player.stop(),
            BrowserSongPlayer::Ay(player) => player.stop(),
            BrowserSongPlayer::Sndh(player) => player.stop(),
        }
    }

    /// Get current playback state.
    pub fn state(&self) -> PlaybackState {
        match self {
            BrowserSongPlayer::Ym(player) => player.state(),
            BrowserSongPlayer::Arkos(player) => player.state(),
            BrowserSongPlayer::Ay(player) => player.state(),
            BrowserSongPlayer::Sndh(player) => player.state(),
        }
    }

    /// Get current frame position.
    pub fn frame_position(&self) -> usize {
        match self {
            BrowserSongPlayer::Ym(player) => player.get_current_frame(),
            BrowserSongPlayer::Arkos(player) => player.frame_position(),
            BrowserSongPlayer::Ay(player) => player.frame_position(),
            BrowserSongPlayer::Sndh(player) => player.frame_position(),
        }
    }

    /// Get total frame count.
    pub fn frame_count(&self) -> usize {
        match self {
            BrowserSongPlayer::Ym(player) => player.frame_count(),
            BrowserSongPlayer::Arkos(player) => player.frame_count(),
            BrowserSongPlayer::Ay(player) => player.frame_count(),
            BrowserSongPlayer::Sndh(player) => player.frame_count(),
        }
    }

    /// Get playback position as percentage (0.0 to 1.0).
    pub fn playback_position(&self) -> f32 {
        match self {
            BrowserSongPlayer::Ym(player) => player.playback_position(),
            BrowserSongPlayer::Arkos(player) => player.playback_position(),
            BrowserSongPlayer::Ay(player) => player.playback_position(),
            BrowserSongPlayer::Sndh(player) => player.playback_position(),
        }
    }

    /// Generate audio samples.
    pub fn generate_samples(&mut self, count: usize) -> Vec<f32> {
        match self {
            BrowserSongPlayer::Ym(player) => player.generate_samples(count),
            BrowserSongPlayer::Arkos(player) => player.generate_samples(count),
            BrowserSongPlayer::Ay(player) => player.generate_samples(count),
            BrowserSongPlayer::Sndh(player) => player.generate_samples(count),
        }
    }

    /// Generate audio samples into a pre-allocated buffer.
    pub fn generate_samples_into(&mut self, buffer: &mut [f32]) {
        match self {
            BrowserSongPlayer::Ym(player) => player.generate_samples_into(buffer),
            BrowserSongPlayer::Arkos(player) => player.generate_samples_into(buffer),
            BrowserSongPlayer::Ay(player) => player.generate_samples_into(buffer),
            BrowserSongPlayer::Sndh(player) => player.generate_samples_into(buffer),
        }
    }

    /// Mute or unmute a channel.
    pub fn set_channel_mute(&mut self, channel: usize, mute: bool) {
        match self {
            BrowserSongPlayer::Ym(player) => player.set_channel_mute(channel, mute),
            BrowserSongPlayer::Arkos(player) => player.set_channel_mute(channel, mute),
            BrowserSongPlayer::Ay(player) => player.set_channel_mute(channel, mute),
            BrowserSongPlayer::Sndh(player) => player.set_channel_mute(channel, mute),
        }
    }

    /// Check if a channel is muted.
    pub fn is_channel_muted(&self, channel: usize) -> bool {
        match self {
            BrowserSongPlayer::Ym(player) => player.is_channel_muted(channel),
            BrowserSongPlayer::Arkos(player) => player.is_channel_muted(channel),
            BrowserSongPlayer::Ay(player) => player.is_channel_muted(channel),
            BrowserSongPlayer::Sndh(player) => player.is_channel_muted(channel),
        }
    }

    /// Dump current PSG register values.
    pub fn dump_registers(&self) -> [u8; 16] {
        match self {
            BrowserSongPlayer::Ym(player) => player.get_chip().dump_registers(),
            BrowserSongPlayer::Arkos(player) => player.dump_registers(),
            BrowserSongPlayer::Ay(player) => player.dump_registers(),
            BrowserSongPlayer::Sndh(player) => player.dump_registers(),
        }
    }

    /// Enable or disable the color filter.
    pub fn set_color_filter(&mut self, enabled: bool) {
        match self {
            BrowserSongPlayer::Ym(player) => player.get_chip_mut().set_color_filter(enabled),
            BrowserSongPlayer::Arkos(player) => player.set_color_filter(enabled),
            BrowserSongPlayer::Ay(player) => player.set_color_filter(enabled),
            BrowserSongPlayer::Sndh(player) => player.set_color_filter(enabled),
        }
    }
}
