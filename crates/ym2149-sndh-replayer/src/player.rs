//! SNDH player implementation.
//!
//! This module provides the main `SndhPlayer` struct that handles SNDH
//! file playback using the Atari ST machine emulation.

use crate::error::{Result, SndhError};
use crate::machine::AtariMachine;
use crate::parser::{SndhFile, SubsongInfo};
use ym2149::Ym2149Backend;
use ym2149_common::{BasicMetadata, ChiptunePlayer, ChiptunePlayerBase, PlaybackState};

/// SNDH file player.
///
/// Handles playback of SNDH files using Atari ST machine emulation.
/// SNDH files contain native 68000 code that runs on the emulated machine.
///
/// # Example
///
/// ```rust,ignore
/// use ym2149_sndh_replayer::SndhPlayer;
///
/// let data = std::fs::read("music.sndh")?;
/// let mut player = SndhPlayer::new(&data, 44100)?;
///
/// // Initialize first subsong
/// player.init_subsong(1)?;
///
/// // Generate audio
/// let mut buffer = vec![0.0f32; 882]; // ~20ms at 44100Hz
/// player.generate_samples_into(&mut buffer);
/// ```
pub struct SndhPlayer {
    /// Atari ST machine
    machine: AtariMachine,
    /// Parsed SNDH file
    sndh: SndhFile,
    /// Current playback state
    state: PlaybackState,
    /// Player metadata
    metadata: BasicMetadata,
    /// Host sample rate
    sample_rate: u32,
    /// Samples per player tick (at player_rate Hz)
    samples_per_tick: u32,
    /// Current sample position within tick
    inner_sample_pos: i32,
    /// Current frame counter
    frame: u32,
    /// Total frame count for current subsong (0 = unknown)
    frame_count: u32,
    /// Loop counter
    loop_count: u32,
    /// Current subsong (1-based)
    current_subsong: usize,
    /// Max cycles allowed per play call (configurable for heavy drivers)
    play_cycle_budget: usize,
    /// Disable warmup/prime phase (env flag)
    warmup_enabled: bool,
}

impl SndhPlayer {
    /// Create a new SNDH player from raw file data.
    ///
    /// # Arguments
    ///
    /// * `data` - Raw SNDH file data (may be ICE! compressed)
    /// * `sample_rate` - Output sample rate (e.g., 44100)
    ///
    /// # Returns
    ///
    /// A new player ready for subsong initialization.
    pub fn new(data: &[u8], sample_rate: u32) -> Result<Self> {
        let sndh = SndhFile::parse(data)?;

        let metadata = BasicMetadata {
            title: sndh.metadata.title.clone().unwrap_or_default(),
            author: sndh.metadata.author.clone().unwrap_or_default(),
            comments: String::new(),
            format: "SNDH".to_string(),
            frame_count: None, // Varies by subsong
            frame_rate: sndh.metadata.player_rate,
            loop_frame: None,
        };

        let samples_per_tick = sample_rate / sndh.metadata.player_rate;

        let play_cycle_budget = std::env::var("YM2149_PLAY_CYCLES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(400_000);
        let warmup_enabled = std::env::var_os("YM2149_NO_WARMUP").is_none();

        Ok(Self {
            machine: AtariMachine::new(sample_rate),
            sndh,
            state: PlaybackState::Stopped,
            metadata,
            sample_rate,
            samples_per_tick,
            inner_sample_pos: 0,
            frame: 0,
            frame_count: 0,
            loop_count: 0,
            current_subsong: 0,
            play_cycle_budget,
            warmup_enabled,
        })
    }

    /// Initialize a specific subsong.
    ///
    /// # Arguments
    ///
    /// * `subsong_id` - Subsong number (1-based)
    ///
    /// # Returns
    ///
    /// Ok(()) if initialization succeeded, or an error.
    pub fn init_subsong(&mut self, subsong_id: usize) -> Result<()> {
        if subsong_id < 1 || subsong_id > self.sndh.metadata.subsong_count {
            return Err(SndhError::InvalidSubsong {
                index: subsong_id,
                available: self.sndh.metadata.subsong_count,
            });
        }

        // Reset machine
        self.machine.reset();

        // Upload SNDH data
        let upload_addr = self.machine.sndh_upload_addr();
        self.machine.upload(self.sndh.raw_data(), upload_addr)?;

        // Call init routine (entry point + 0) with subsong in D0
        let success = self.machine.jsr(upload_addr, subsong_id as u32)?;
        if !success {
            return Err(SndhError::CpuError(
                "Init routine did not complete successfully".to_string(),
            ));
        }

        // Setup playback state
        self.current_subsong = subsong_id;
        self.frame = 0;
        self.loop_count = 0;

        // CRITICAL: Call play routine IMMEDIATELY after init to set up registers
        // before any samples are generated. This matches psgplay behavior where
        // the timer fires immediately after init.
        let _ = self
            .machine
            .jsr_limited(upload_addr + 8, 0, self.play_cycle_budget);
        self.frame += 1;

        // Set inner_sample_pos to full tick so we don't call play again immediately
        self.inner_sample_pos = self.samples_per_tick as i32;

        // Let hardware timers run for one player tick (20 ms @50 Hz)
        // so timer-driven effects are "primed".
        if self.warmup_enabled {
            for _ in 0..self.samples_per_tick {
                let _ = self.machine.compute_sample_stereo();
            }
        }

        // Calculate frame count from duration
        if let Some(info) = self.sndh.get_subsong_info(subsong_id, self.sample_rate) {
            self.frame_count = info.player_tick_count;
        } else {
            self.frame_count = 0; // Unknown duration
        }

        self.state = PlaybackState::Stopped;
        Ok(())
    }

    /// Get information about a specific subsong.
    pub fn get_subsong_info(&self, subsong_id: usize) -> Option<SubsongInfo> {
        self.sndh.get_subsong_info(subsong_id, self.sample_rate)
    }

    /// Get the number of subsongs.
    pub fn subsong_count(&self) -> usize {
        self.sndh.metadata.subsong_count
    }

    /// Get the default subsong (1-based).
    pub fn default_subsong(&self) -> usize {
        self.sndh.metadata.default_subsong
    }

    /// Get the current subsong (1-based), or 0 if not initialized.
    pub fn current_subsong(&self) -> usize {
        self.current_subsong
    }

    /// Get the number of times the song has looped.
    pub fn loop_count(&self) -> u32 {
        self.loop_count
    }

    /// Get the player tick rate in Hz.
    pub fn player_rate(&self) -> u32 {
        self.sndh.metadata.player_rate
    }

    /// Get reference to the YM2149 chip.
    pub fn ym2149(&self) -> &ym2149::Ym2149 {
        self.machine.ym2149()
    }

    /// Get mutable reference to the YM2149 chip (for channel muting).
    pub fn ym2149_mut(&mut self) -> &mut ym2149::Ym2149 {
        self.machine.ym2149_mut()
    }

    /// Render audio into a buffer of interleaved stereo i16 samples.
    ///
    /// Buffer length must be even (pairs of left/right samples).
    /// Returns loop count.
    pub fn render_i16_stereo(&mut self, buffer: &mut [i16]) -> u32 {
        self.render_into_stereo(buffer, 0i16, |left, right| (left, right))
    }

    /// Render audio into a buffer of interleaved stereo f32 samples.
    ///
    /// Buffer length must be even (pairs of left/right samples).
    /// Returns loop count.
    pub fn render_f32_stereo(&mut self, buffer: &mut [f32]) -> u32 {
        self.render_into_stereo(buffer, 0.0f32, |left, right| {
            (left as f32 / 32768.0, right as f32 / 32768.0)
        })
    }

    fn render_into_stereo<T: Copy>(
        &mut self,
        buffer: &mut [T],
        silence: T,
        mut convert: impl FnMut(i16, i16) -> (T, T),
    ) -> u32 {
        if self.state != PlaybackState::Playing || self.current_subsong == 0 {
            buffer.fill(silence);
            return self.loop_count;
        }

        let upload_addr = self.machine.sndh_upload_addr();

        // Process pairs of samples (left, right)
        for chunk in buffer.chunks_exact_mut(2) {
            self.inner_sample_pos -= 1;

            // Call player tick routine when needed
            if self.inner_sample_pos <= 0 {
                // Call play routine (entry point + 8) with limited cycles (or unlimited if budget==0)
                if self.play_cycle_budget == 0 {
                    let _ = self.machine.jsr(upload_addr + 8, 0);
                } else {
                    let _ = self
                        .machine
                        .jsr_limited(upload_addr + 8, 0, self.play_cycle_budget);
                }
                self.inner_sample_pos = self.samples_per_tick as i32;
                self.frame += 1;

                // Check for loop
                if self.frame_count > 0 && self.frame >= self.frame_count {
                    self.loop_count += 1;
                }
            }

            // Generate stereo audio sample
            let (left, right) = self.machine.compute_sample_stereo();
            let (out_left, out_right) = convert(left, right);
            chunk[0] = out_left;
            chunk[1] = out_right;
        }

        self.loop_count
    }
}

impl ChiptunePlayerBase for SndhPlayer {
    fn play(&mut self) {
        if self.current_subsong > 0 {
            self.state = PlaybackState::Playing;
        }
    }

    fn pause(&mut self) {
        if self.state == PlaybackState::Playing {
            self.state = PlaybackState::Paused;
        }
    }

    fn stop(&mut self) {
        self.state = PlaybackState::Stopped;
        self.frame = 0;
        self.inner_sample_pos = 0;
        self.loop_count = 0;
    }

    fn state(&self) -> PlaybackState {
        self.state
    }

    fn generate_samples_into(&mut self, buffer: &mut [f32]) {
        // Generate stereo and mix down to mono for trait compatibility
        let frame_count = buffer.len();
        let mut stereo_buffer = vec![0.0f32; frame_count * 2];
        self.render_f32_stereo(&mut stereo_buffer);
        for (i, sample) in buffer.iter_mut().enumerate() {
            *sample = (stereo_buffer[i * 2] + stereo_buffer[i * 2 + 1]) * 0.5;
        }
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn set_channel_mute(&mut self, channel: usize, mute: bool) {
        self.machine.ym2149_mut().set_channel_mute(channel, mute);
    }

    fn is_channel_muted(&self, channel: usize) -> bool {
        self.machine.ym2149().is_channel_muted(channel)
    }

    fn playback_position(&self) -> f32 {
        // SNDH doesn't have reliable frame count tracking
        0.0
    }

    fn subsong_count(&self) -> usize {
        SndhPlayer::subsong_count(self)
    }

    fn current_subsong(&self) -> usize {
        SndhPlayer::current_subsong(self)
    }

    fn set_subsong(&mut self, index: usize) -> bool {
        if index >= 1 && index <= self.subsong_count() && self.init_subsong(index).is_ok() {
            self.state = PlaybackState::Playing;
            true
        } else {
            false
        }
    }
}

impl ChiptunePlayer for SndhPlayer {
    type Metadata = BasicMetadata;

    fn metadata(&self) -> &Self::Metadata {
        &self.metadata
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_minimal_sndh() -> Vec<u8> {
        // Create a minimal valid SNDH that does nothing
        // This is just for testing the parser - actual playback needs real SNDH
        let mut data = vec![0u8; 64];
        data[0] = 0x60; // BRA.s
        data[1] = 0x3E; // offset to byte 64
        data[12..16].copy_from_slice(b"SNDH");
        data[16..20].copy_from_slice(b"HDNS");
        data
    }

    #[test]
    fn test_player_creation() {
        let data = make_minimal_sndh();
        let player = SndhPlayer::new(&data, 44100);
        assert!(player.is_ok());
    }

    #[test]
    fn test_metadata_access() {
        let data = make_minimal_sndh();
        let player = SndhPlayer::new(&data, 44100).unwrap();
        assert_eq!(player.metadata().format, "SNDH");
        assert_eq!(player.subsong_count(), 1);
    }
}
