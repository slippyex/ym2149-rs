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
    /// Cached STE detection (preserved across subsong changes)
    uses_ste: bool,
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

        // Warm-up: Generate audio to detect STE hardware usage at runtime.
        // Some drivers don't enable DMA until actual playback starts.
        // We generate ~500ms of audio and discard the output.
        {
            let mut discard_buffer = [0.0f32; 22050]; // ~500ms at 44100Hz (stereo = 11025 frames)
            player.render_f32_stereo(&mut discard_buffer);
        }

        // Debug: Check what flags are in the metadata
        let flags = player.sndh_flags();
        web_sys::console::log_1(&format!("SNDH metadata flags - ste: {}, lmc: {}, stereo: {}, dma_rate: {:?}",
            flags.ste,
            flags.lmc,
            flags.stereo,
            flags.dma_rate
        ).into());
        let dac_used = player.was_ste_dac_used();
        web_sys::console::log_1(&format!("SNDH was_ste_dac_used after warm-up: {dac_used}").into());

        // Capture STE detection state BEFORE re-init (reset would clear it)
        let uses_ste = player.uses_ste_features();
        web_sys::console::log_1(&format!("SNDH uses_ste_features(): {uses_ste}").into());

        // Re-initialize to reset position to beginning (clean state)
        let _ = player.init_subsong(default_subsong);

        let metadata = metadata_from_player(&player);
        web_sys::console::log_1(&format!("SNDH channel_count: {}", if uses_ste { 5 } else { 3 }).into());
        Ok((Self { player, uses_ste }, metadata))
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

    /// Get the number of times the song has looped.
    pub fn loop_count(&self) -> u32 {
        self.player.loop_count()
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
    ///
    /// SNDH has 5 logical channels:
    /// - 0, 1, 2: YM2149 channels A, B, C
    /// - 3: STE DAC Left
    /// - 4: STE DAC Right
    pub fn set_channel_mute(&mut self, channel: usize, mute: bool) {
        match channel {
            0..=2 => {
                // YM2149 channels
                ChiptunePlayerBase::set_channel_mute(&mut self.player, channel, mute);
            }
            3 => {
                // DAC Left
                self.player.set_dac_mute_left(mute);
            }
            4 => {
                // DAC Right
                self.player.set_dac_mute_right(mute);
            }
            _ => {}
        }
    }

    /// Check if a channel is muted.
    ///
    /// SNDH has 5 logical channels:
    /// - 0, 1, 2: YM2149 channels A, B, C
    /// - 3: STE DAC Left
    /// - 4: STE DAC Right
    pub fn is_channel_muted(&self, channel: usize) -> bool {
        match channel {
            0..=2 => ChiptunePlayerBase::is_channel_muted(&self.player, channel),
            3 => self.player.is_dac_left_muted(),
            4 => self.player.is_dac_right_muted(),
            _ => false,
        }
    }

    /// Get the number of channels.
    ///
    /// Always returns 5 for SNDH (3 YM2149 + 2 DAC).
    /// DAC channels will show zero activity for non-STE songs.
    pub fn channel_count(&self) -> usize {
        5 // Always show all channels: 3 YM + 2 DAC (L/R)
    }

    /// Check if this SNDH uses STE hardware features.
    pub fn uses_ste_features(&self) -> bool {
        self.uses_ste
    }

    /// Get current DAC levels for visualization (normalized 0.0 to 1.0).
    ///
    /// Returns (left, right) amplitude values.
    pub fn get_dac_levels(&self) -> (f32, f32) {
        self.player.get_dac_levels()
    }

    /// Dump current PSG register values.
    pub fn dump_registers(&self) -> [u8; 16] {
        self.player.ym2149().dump_registers()
    }

    /// Get current per-channel audio outputs.
    ///
    /// Returns the actual audio output values (A, B, C) updated at sample rate.
    pub fn get_channel_outputs(&self) -> (f32, f32, f32) {
        self.player.ym2149().get_channel_outputs()
    }

    /// Generate samples with per-sample channel outputs for visualization.
    ///
    /// Fills the mono buffer with mixed samples and channels buffer with
    /// per-sample channel outputs. For SNDH, we capture YM2149 channels
    /// and STE DAC levels.
    ///
    /// Channel layout: [A, B, C, DAC_L, DAC_R] (always 5 channels for SNDH).
    pub fn generate_samples_with_channels_into(&mut self, mono: &mut [f32], channels: &mut [f32]) {
        let channel_count = self.channel_count();

        // Generate samples in small batches and capture channel state after each
        // Using batch size of 1 stereo frame (2 samples) for best accuracy
        let mut stereo_buf = [0.0f32; 2];
        let mut idx = 0;

        while idx < mono.len() {
            // Render one stereo frame
            self.player.render_f32_stereo(&mut stereo_buf);
            mono[idx] = (stereo_buf[0] + stereo_buf[1]) * 0.5;

            // Capture YM2149 channel outputs
            let (a, b, c) = self.player.ym2149().get_channel_outputs();
            let base = idx * channel_count;
            channels[base] = a;
            channels[base + 1] = b;
            channels[base + 2] = c;

            // Capture DAC levels
            let (dac_l, dac_r) = self.player.get_dac_levels();
            channels[base + 3] = dac_l;
            channels[base + 4] = dac_r;

            idx += 1;
        }
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

    /// Get LMC1992 master volume in dB (-80 to 0).
    pub fn lmc1992_master_volume_db(&self) -> i8 {
        self.player.lmc1992_master_volume_db()
    }

    /// Get LMC1992 left volume in dB (-40 to 0).
    pub fn lmc1992_left_volume_db(&self) -> i8 {
        self.player.lmc1992_left_volume_db()
    }

    /// Get LMC1992 right volume in dB (-40 to 0).
    pub fn lmc1992_right_volume_db(&self) -> i8 {
        self.player.lmc1992_right_volume_db()
    }

    /// Get LMC1992 bass in dB (-12 to +12).
    pub fn lmc1992_bass_db(&self) -> i8 {
        self.player.lmc1992_bass_db()
    }

    /// Get LMC1992 treble in dB (-12 to +12).
    pub fn lmc1992_treble_db(&self) -> i8 {
        self.player.lmc1992_treble_db()
    }

    /// Get LMC1992 master volume raw value (0-40).
    pub fn lmc1992_master_volume_raw(&self) -> u8 {
        self.player.lmc1992_master_volume_raw()
    }

    /// Get LMC1992 left volume raw value (0-20).
    pub fn lmc1992_left_volume_raw(&self) -> u8 {
        self.player.lmc1992_left_volume_raw()
    }

    /// Get LMC1992 right volume raw value (0-20).
    pub fn lmc1992_right_volume_raw(&self) -> u8 {
        self.player.lmc1992_right_volume_raw()
    }

    /// Get LMC1992 bass raw value (0-12).
    pub fn lmc1992_bass_raw(&self) -> u8 {
        self.player.lmc1992_bass_raw()
    }

    /// Get LMC1992 treble raw value (0-12).
    pub fn lmc1992_treble_raw(&self) -> u8 {
        self.player.lmc1992_treble_raw()
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
