//! Arkos Tracker song player.
//!
//! This is the main player that manages song-level playback:
//! - Pattern/position navigation
//! - Tick management (50Hz replay frequency)
//! - Channel player orchestration
//! - PSG register updates
//! - Sample generation
//!
//! # Module Organization
//!
//! - [`sample_voice`] - Sample/digi-drum mixing
//! - [`psg_output`] - PSG register writing
//! - [`tick`] - Tick processing and song advancement
//! - [`chiptune_player`] - ChiptunePlayer trait implementation
//!
//! # Example
//!
//! ```no_run
//! use ym2149_arkos_replayer::{load_aks, ArkosPlayer};
//!
//! let data = std::fs::read("song.aks")?;
//! let song = load_aks(&data)?;
//! let mut player = ArkosPlayer::new(song, 0)?;
//!
//! player.play()?;
//! let samples = player.generate_samples(44100); // 1 second of audio
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod chiptune_player;
mod psg_output;
mod sample_voice;
mod tick;

pub use chiptune_player::ArkosMetadata;

#[cfg(all(test, feature = "extended-tests"))]
mod tests;

use std::sync::Arc;

#[cfg(all(test, feature = "extended-tests"))]
use crate::channel_player::ChannelFrame;
use crate::channel_player::ChannelPlayer;
use crate::effect_context::EffectContext;
use crate::error::{ArkosError, Result};
use crate::format::{AksSong, SongMetadata};
use ym2149::Ym2149Backend;
use ym2149::ym2149::PsgBank;
use ym2149::ym2149::Ym2149;

use sample_voice::{HardwareEnvelopeState, SampleVoiceMixer};
use tick::{TickContext, determine_speed_for_location};

// Re-export for tests
#[cfg(all(test, feature = "extended-tests"))]
pub(crate) use tick::resolve_cell;

/// Arkos Tracker song player.
///
/// Manages multiple channels, patterns, and PSG chips.
pub struct ArkosPlayer {
    /// The song being played
    pub(crate) song: Arc<AksSong>,
    /// Precomputed effect context state
    pub(crate) effect_context: EffectContext,
    /// PSG bank (multiple chips)
    psg_bank: PsgBank,
    /// Current subsong index
    pub(crate) subsong_index: usize,
    /// Whether currently playing
    is_playing: bool,

    // Playback state
    /// Channel players (one per channel)
    pub(crate) channel_players: Vec<ChannelPlayer>,
    /// Current position in song
    pub(crate) current_position: usize,
    /// Current line in pattern
    pub(crate) current_line: usize,
    /// Current speed (ticks per line)
    pub(crate) current_speed: u8,
    /// Current tick counter (0..speed)
    current_tick: u8,

    // Sample generation timing
    /// Sample counter for tick timing
    sample_counter: f32,
    /// How many samples per tick
    samples_per_tick: f32,
    /// Active sample voices per channel
    sample_voices: Vec<SampleVoiceMixer>,
    /// Last hardware envelope shape per PSG (for avoiding unwanted retrigs)
    hardware_envelope_state: Vec<HardwareEnvelopeState>,
    /// Output sample rate
    output_sample_rate: f32,
    /// Cached metadata for ChiptunePlayer trait
    cached_metadata: ArkosMetadata,
}

impl ArkosPlayer {
    /// Create a new Arkos player.
    ///
    /// # Arguments
    ///
    /// * `song` - The parsed AKS song
    /// * `subsong_index` - Which subsong to play (0-based)
    ///
    /// # Errors
    ///
    /// Returns an error if the subsong index is out of range or PSG configuration is invalid.
    pub fn new(song: AksSong, subsong_index: usize) -> Result<Self> {
        if subsong_index >= song.subsongs.len() {
            return Err(ArkosError::InvalidSubsong {
                index: subsong_index,
                available: song.subsongs.len(),
            });
        }

        let song = Arc::new(song);
        let effect_context = EffectContext::build(&song, subsong_index)?;
        let subsong = &song.subsongs[subsong_index];

        // Create PSG bank with frequencies from subsong
        let frequencies: Vec<u32> = subsong.psgs.iter().map(|p| p.psg_frequency).collect();

        let psg_bank = if frequencies.is_empty() {
            return Err(ArkosError::PsgError(
                "No PSGs defined in subsong".to_string(),
            ));
        } else {
            PsgBank::new_with_frequencies(frequencies)
        };

        // Calculate samples per tick (how many samples between pattern updates)
        // replay_frequency_hz is the pattern update rate (e.g., 50 Hz)
        // PSG output sample rate is typically 44100 Hz
        let output_sample_rate = 44100.0;
        let samples_per_tick = output_sample_rate / subsong.replay_frequency_hz;

        // Create channel players (3 channels per PSG)
        let channel_count = subsong.psgs.len() * 3;
        let mut channel_players = Vec::with_capacity(channel_count);

        for channel_idx in 0..channel_count {
            // Get PSG for this channel
            let psg_idx = channel_idx / 3;
            let psg = &subsong.psgs[psg_idx];

            let channel_player = ChannelPlayer::new(
                channel_idx,
                Arc::clone(&song),
                psg.psg_frequency as f32,
                psg.reference_frequency,
                psg.sample_player_frequency as f32,
            );

            channel_players.push(channel_player);
        }

        let initial_speed = subsong.initial_speed;
        let sample_voices = vec![SampleVoiceMixer::default(); channel_count];

        let hardware_envelope_state = vec![HardwareEnvelopeState::default(); psg_bank.psg_count()];

        // Calculate cached metadata
        let song_meta = &song.metadata;
        let estimated_lines: usize = subsong.positions.iter().map(|pos| pos.height).sum();
        let cached_metadata = ArkosMetadata {
            title: song_meta.title.clone(),
            author: if song_meta.author.is_empty() {
                song_meta.composer.clone()
            } else {
                song_meta.author.clone()
            },
            comments: song_meta.comments.clone(),
            estimated_lines,
            replay_frequency: subsong.replay_frequency_hz,
        };

        let mut player = Self {
            song,
            effect_context,
            psg_bank,
            subsong_index,
            is_playing: false,
            channel_players,
            current_position: 0,
            current_line: 0,
            current_speed: initial_speed,
            current_tick: 0,
            sample_counter: 0.0,
            samples_per_tick,
            sample_voices,
            hardware_envelope_state,
            output_sample_rate,
            cached_metadata,
        };

        player.current_speed = determine_speed_for_location(&player.song, subsong_index, 0, 0);

        Ok(player)
    }

    /// Start playback.
    pub fn play(&mut self) -> Result<()> {
        self.is_playing = true;
        Ok(())
    }

    /// Pause playback.
    pub fn pause(&mut self) -> Result<()> {
        self.is_playing = false;
        Ok(())
    }

    /// Stop playback and reset.
    pub fn stop(&mut self) -> Result<()> {
        self.is_playing = false;
        self.psg_bank.reset();
        for state in &mut self.hardware_envelope_state {
            *state = HardwareEnvelopeState::default();
        }

        // Reset playback state
        self.current_position = 0;
        self.current_line = 0;
        self.current_tick = 0;
        self.sample_counter = 0.0;
        self.current_speed = determine_speed_for_location(&self.song, self.subsong_index, 0, 0);

        // Reset all channels
        for channel in &mut self.channel_players {
            channel.stop_sound();
        }

        Ok(())
    }

    /// Output sample rate in Hz.
    pub fn output_sample_rate(&self) -> f32 {
        self.output_sample_rate
    }

    /// Samples produced per tick (line advancement).
    pub fn samples_per_tick(&self) -> f32 {
        self.samples_per_tick
    }

    /// Number of PSG chips in this song.
    pub fn chip_count(&self) -> usize {
        self.psg_bank.psg_count()
    }

    /// Get a reference to a PSG chip by index.
    pub fn chip(&self, index: usize) -> Option<&Ym2149> {
        if index < self.psg_bank.psg_count() {
            Some(self.psg_bank.get_chip(index))
        } else {
            None
        }
    }

    /// Get mutable access to a PSG chip by index.
    pub fn chip_mut(&mut self, index: usize) -> Option<&mut Ym2149> {
        if index < self.psg_bank.psg_count() {
            Some(self.psg_bank.get_chip_mut(index))
        } else {
            None
        }
    }

    /// Mute or unmute a global channel (0 = PSG0:A, 1 = PSG0:B, 2 = PSG0:C, 3 = PSG1:A, ...).
    pub fn set_channel_mute(&mut self, channel: usize, mute: bool) {
        let psg_idx = channel / 3;
        let channel_in_psg = channel % 3;
        if let Some(chip) = self.chip_mut(psg_idx) {
            chip.set_channel_mute(channel_in_psg, mute);
        }
    }

    /// Check whether a global channel is muted.
    pub fn is_channel_muted(&self, channel: usize) -> bool {
        let psg_idx = channel / 3;
        let channel_in_psg = channel % 3;
        self.chip(psg_idx)
            .map(|chip| chip.is_channel_muted(channel_in_psg))
            .unwrap_or(false)
    }

    /// Get current absolute tick (line * speed + tick).
    pub fn current_tick_index(&self) -> usize {
        let line_offset = self.calculate_line_offset();
        line_offset
            .saturating_mul(self.current_speed.max(1) as usize)
            .saturating_add(self.current_tick as usize)
    }

    /// Estimated total ticks (lines * nominal speed).
    pub fn estimated_total_ticks(&self) -> usize {
        let subsong = &self.song.subsongs[self.subsong_index];
        let total_lines: usize = subsong.positions.iter().map(|pos| pos.height).sum();
        total_lines.saturating_mul(subsong.initial_speed.max(1) as usize)
    }

    /// Access song metadata.
    pub fn metadata(&self) -> &SongMetadata {
        &self.song.metadata
    }

    /// Replay frequency in Hz.
    pub fn replay_frequency_hz(&self) -> f32 {
        self.song.subsongs[self.subsong_index].replay_frequency_hz
    }

    fn calculate_line_offset(&self) -> usize {
        let subsong = &self.song.subsongs[self.subsong_index];
        let mut total_lines = 0usize;
        for pos_idx in 0..self.current_position.min(subsong.positions.len()) {
            total_lines += subsong.positions[pos_idx].height;
        }
        total_lines
            + self.current_line.min(
                subsong
                    .positions
                    .get(self.current_position)
                    .map(|pos| pos.height)
                    .unwrap_or(0),
            )
    }

    /// Get number of PSG chips.
    pub fn psg_count(&self) -> usize {
        self.psg_bank.psg_count()
    }

    /// Get number of channels.
    pub fn channel_count(&self) -> usize {
        self.channel_players.len()
    }

    /// Check if player is currently playing.
    pub fn is_playing(&self) -> bool {
        self.is_playing
    }

    /// Generate audio samples.
    ///
    /// # Arguments
    ///
    /// * `count` - Number of samples to generate
    ///
    /// # Returns
    ///
    /// Vector of f32 samples in range -1.0..1.0
    pub fn generate_samples(&mut self, count: usize) -> Vec<f32> {
        let mut buffer = vec![0.0f32; count];
        self.generate_samples_into(&mut buffer);
        buffer
    }

    /// Generate audio directly into provided buffer (avoids reallocations on hot path).
    pub fn generate_samples_into(&mut self, buffer: &mut [f32]) {
        if buffer.is_empty() {
            return;
        }

        buffer.fill(0.0);

        if !self.is_playing {
            return;
        }

        let mut sample_pos = 0;
        let total_samples = buffer.len();

        while sample_pos < total_samples {
            let samples_until_tick = (self.samples_per_tick - self.sample_counter).ceil() as usize;
            let samples_to_generate = samples_until_tick.min(total_samples - sample_pos);

            let mut psg_buffers = Vec::with_capacity(self.psg_bank.psg_count());
            for psg_idx in 0..self.psg_bank.psg_count() {
                let samples = self
                    .psg_bank
                    .get_chip_mut(psg_idx)
                    .generate_samples(samples_to_generate);
                psg_buffers.push(samples);
            }

            for sample_idx in 0..samples_to_generate {
                let mut mixed_sample = 0.0;
                for psg_buffer in &psg_buffers {
                    mixed_sample += psg_buffer[sample_idx];
                }
                buffer[sample_pos + sample_idx] = mixed_sample / self.psg_bank.psg_count() as f32;
            }

            self.mix_active_samples(sample_pos, samples_to_generate, buffer);

            sample_pos += samples_to_generate;
            self.sample_counter += samples_to_generate as f32;

            if self.sample_counter >= self.samples_per_tick {
                self.sample_counter -= self.samples_per_tick;
                self.process_tick();
            }
        }
    }

    fn mix_active_samples(&mut self, start: usize, len: usize, buffer: &mut [f32]) {
        if len == 0 {
            return;
        }

        let segment = &mut buffer[start..start + len];
        for voice in &mut self.sample_voices {
            voice.mix_into(segment);
        }
    }

    /// Process one tick of playback.
    fn process_tick(&mut self) {
        let mut ctx = TickContext {
            song: &self.song,
            effect_context: &self.effect_context,
            psg_bank: &mut self.psg_bank,
            subsong_index: self.subsong_index,
            channel_players: &mut self.channel_players,
            current_position: &mut self.current_position,
            current_line: &mut self.current_line,
            current_speed: &mut self.current_speed,
            current_tick: &mut self.current_tick,
            sample_voices: &mut self.sample_voices,
            hardware_envelope_state: &mut self.hardware_envelope_state,
            output_sample_rate: self.output_sample_rate,
        };
        ctx.process_tick();
    }

    /// Capture tick frames for testing (extended-tests feature only).
    #[cfg(all(test, feature = "extended-tests"))]
    pub(crate) fn capture_tick_frames(&mut self) -> Vec<ChannelFrame> {
        let mut captured = Vec::new();
        let mut ctx = TickContext {
            song: &self.song,
            effect_context: &self.effect_context,
            psg_bank: &mut self.psg_bank,
            subsong_index: self.subsong_index,
            channel_players: &mut self.channel_players,
            current_position: &mut self.current_position,
            current_line: &mut self.current_line,
            current_speed: &mut self.current_speed,
            current_tick: &mut self.current_tick,
            sample_voices: &mut self.sample_voices,
            hardware_envelope_state: &mut self.hardware_envelope_state,
            output_sample_rate: self.output_sample_rate,
        };
        ctx.run_tick(|frames| {
            captured = frames.to_vec();
        });
        captured
    }

    /// Get debug information about channel states.
    ///
    /// Returns (note, period, volume) for each channel.
    pub fn debug_channel_states(&self) -> Vec<(u8, u16, u8)> {
        // This would need access to channel player internals
        // For now, return dummy data
        vec![(0, 0, 0); self.channel_players.len()]
    }
}
