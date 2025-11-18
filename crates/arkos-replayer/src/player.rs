//! Arkos Tracker song player
//!
//! This is the main player that manages song-level playback:
//! - Pattern/position navigation
//! - Tick management (50Hz replay frequency)
//! - Channel player orchestration
//! - PSG register updates
//! - Sample generation

use std::sync::Arc;

use crate::channel_player::{
    ChannelFrame, ChannelOutput, ChannelPlayer, SampleCommand, SamplePlaybackParams,
};
use crate::effect_context::EffectContext;
use crate::error::{ArkosError, Result};
use crate::format::{AksSong, InstrumentType, SongMetadata};
use ym2149::ym2149::PsgBank;
use ym2149::ym2149::Ym2149;

/// Arkos Tracker song player
///
/// Manages multiple channels, patterns, and PSG chips.
pub struct ArkosPlayer {
    /// The song being played
    song: Arc<AksSong>,
    /// Precomputed effect context state
    effect_context: EffectContext,
    /// PSG bank (multiple chips)
    psg_bank: PsgBank,
    /// Current subsong index
    subsong_index: usize,
    /// Whether currently playing
    is_playing: bool,

    // Playback state
    /// Channel players (one per channel)
    channel_players: Vec<ChannelPlayer>,
    /// Current position in song
    current_position: usize,
    /// Current line in pattern
    current_line: usize,
    /// Current speed (ticks per line)
    current_speed: u8,
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
}

impl ArkosPlayer {
    /// Create a new Arkos player
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
        };

        player.current_speed = player.determine_speed_for_location(0, 0);

        Ok(player)
    }

    fn build_frames(
        &mut self,
        subsong_index: usize,
        position_count: usize,
        is_first_tick: bool,
        still_within_line: bool,
    ) -> Vec<ChannelFrame> {
        let mut frames = Vec::with_capacity(self.channel_players.len());

        if is_first_tick {
            for (channel_idx, channel_player) in self.channel_players.iter_mut().enumerate() {
                if self.current_position >= position_count {
                    frames.push(channel_player.play_frame(
                        None,
                        0,
                        is_first_tick,
                        still_within_line,
                    ));
                    continue;
                }

                let (cell, transposition) = {
                    let subsong = &self.song.subsongs[subsong_index];
                    Self::resolve_cell(
                        subsong,
                        self.current_position,
                        self.current_line,
                        channel_idx,
                    )
                };

                if let Some(context) = self
                    .effect_context
                    .line_context(self.current_position, channel_idx, self.current_line)
                    .cloned()
                {
                    channel_player.apply_line_context(&context);
                }

                let frame = channel_player.play_frame(
                    cell,
                    transposition,
                    is_first_tick,
                    still_within_line,
                );
                frames.push(frame);
            }
        } else {
            for channel_player in self.channel_players.iter_mut() {
                frames.push(channel_player.play_frame(None, 0, is_first_tick, still_within_line));
            }
        }

        frames
    }

    fn apply_frame_samples(&mut self, frames: &[ChannelFrame]) {
        for (channel_idx, frame) in frames.iter().enumerate() {
            if let Some(voice) = self.sample_voices.get_mut(channel_idx) {
                voice.apply_command(&frame.sample, self.output_sample_rate);
            }
        }
    }

    fn write_frames_to_psg(&mut self, frames: &[ChannelFrame]) {
        for psg_idx in 0..self.psg_bank.psg_count() {
            let base_channel = psg_idx * 3;

            for ch_in_psg in 0..3 {
                let channel_idx = base_channel + ch_in_psg;
                if channel_idx < frames.len() {
                    self.write_channel_registers(channel_idx, &frames[channel_idx].psg);
                }
            }

            let start = base_channel.min(frames.len());
            self.write_mixer_for_psg(psg_idx, &frames[start..]);
        }
    }

    /// Start playback
    pub fn play(&mut self) -> Result<()> {
        self.is_playing = true;
        Ok(())
    }

    /// Pause playback
    pub fn pause(&mut self) -> Result<()> {
        self.is_playing = false;
        Ok(())
    }

    /// Stop playback and reset
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
        self.current_speed = self.determine_speed_for_location(0, 0);

        // Reset all channels
        for channel in &mut self.channel_players {
            channel.stop_sound();
        }

        Ok(())
    }

    /// Output sample rate in Hz
    pub fn output_sample_rate(&self) -> f32 {
        self.output_sample_rate
    }

    /// Samples produced per tick (line advancement)
    pub fn samples_per_tick(&self) -> f32 {
        self.samples_per_tick
    }

    /// Number of PSG chips in this song
    pub fn chip_count(&self) -> usize {
        self.psg_bank.psg_count()
    }

    /// Get a reference to a PSG chip by index
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

    /// Get current absolute tick (line * speed + tick)
    pub fn current_tick_index(&self) -> usize {
        let line_offset = self.calculate_line_offset();
        line_offset
            .saturating_mul(self.current_speed.max(1) as usize)
            .saturating_add(self.current_tick as usize)
    }

    /// Estimated total ticks (lines * nominal speed)
    pub fn estimated_total_ticks(&self) -> usize {
        let subsong = &self.song.subsongs[self.subsong_index];
        let total_lines: usize = subsong.positions.iter().map(|pos| pos.height).sum();
        total_lines.saturating_mul(subsong.initial_speed.max(1) as usize)
    }

    /// Access song metadata
    pub fn metadata(&self) -> &SongMetadata {
        &self.song.metadata
    }

    /// Replay frequency in Hz
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

    /// Get number of PSG chips
    pub fn psg_count(&self) -> usize {
        self.psg_bank.psg_count()
    }

    /// Get number of channels
    pub fn channel_count(&self) -> usize {
        self.channel_players.len()
    }

    /// Check if player is currently playing
    pub fn is_playing(&self) -> bool {
        self.is_playing
    }

    /// Generate audio samples
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

    /// Generate audio directly into provided buffer (avoids reallocations on hot path)
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

    /// Process one tick of playback
    fn process_tick(&mut self) {
        self.run_tick(|_| {});
    }

    #[cfg(all(test, feature = "extended-tests"))]
    pub(crate) fn capture_tick_frames(&mut self) -> Vec<ChannelFrame> {
        let mut captured = Vec::new();
        self.run_tick(|frames| {
            captured = frames.to_vec();
        });
        captured
    }

    fn run_tick<F>(&mut self, mut observer: F)
    where
        F: FnMut(&[ChannelFrame]),
    {
        let subsong_index = self.subsong_index;
        let is_first_tick = self.current_tick == 0;

        if is_first_tick {
            self.apply_speed_track(subsong_index);
        }

        let still_within_line = true;
        let position_count = self.song.subsongs[subsong_index].positions.len();
        let (loop_start, past_end_position) = self.loop_bounds(subsong_index, position_count);

        if is_first_tick && position_count > 0 && self.current_position >= position_count {
            self.current_position = loop_start;
            self.current_line = 0;
        }

        let frames = self.build_frames(
            subsong_index,
            position_count,
            is_first_tick,
            still_within_line,
        );

        self.apply_frame_samples(&frames);
        observer(&frames);

        if is_first_tick
            && let Some(event_value) = self.read_special_track_value(subsong_index, false)
            && event_value > 0
        {
            self.trigger_event_sample(event_value as usize);
        }

        self.write_frames_to_psg(&frames);

        self.advance_song(loop_start, past_end_position);
    }

    /// Get pattern height for current position (internal version)
    fn get_pattern_height_internal(&self) -> usize {
        let subsong = &self.song.subsongs[self.subsong_index];

        // Get height from position
        if self.current_position < subsong.positions.len() {
            subsong.positions[self.current_position].height
        } else {
            64 // Default fallback
        }
    }

    /// Write channel registers (period, volume) but NOT mixer or noise
    /// This matches C++ SongPlayer::fillWithChannelResult behavior
    fn write_channel_registers(&mut self, channel_idx: usize, output: &ChannelOutput) {
        let psg_idx = channel_idx / 3;
        let channel_in_psg = channel_idx % 3;

        if psg_idx >= self.psg_bank.psg_count() {
            return;
        }

        let psg = self.psg_bank.get_chip_mut(psg_idx);

        // 1. Write tone period (registers 0-5)
        // C++: psgRegistersToFill.setSoftwarePeriod(targetChannel, inputChannelRegisters.getSoftwarePeriod());
        let reg_base = channel_in_psg * 2;
        psg.write_register(reg_base as u8, (output.software_period & 0xFF) as u8);
        psg.write_register(
            (reg_base + 1) as u8,
            ((output.software_period >> 8) & 0x0F) as u8,
        );

        // 2. Write volume (registers 8-10)
        // C++: psgRegistersToFill.setVolume(targetChannel, volume0To16);
        if output.volume == 16 {
            // Hardware envelope mode
            psg.write_register((8 + channel_in_psg) as u8, 0x10);

            // 3. Write hardware envelope period (registers 11-12)
            // C++: psgRegistersToFill.setHardwarePeriod(hardwarePeriod);
            psg.write_register(11, (output.hardware_period & 0xFF) as u8);
            psg.write_register(12, ((output.hardware_period >> 8) & 0xFF) as u8);

            // 4. Write hardware envelope shape (register 13)
            // C++: psgRegistersToFill.setHardwareEnvelopeAndRetrig(hardwareEnvelope, retrig);
            if let Some(state) = self.hardware_envelope_state.get_mut(psg_idx) {
                let shape = output.hardware_envelope & 0x0F;
                let should_write = output.hardware_retrig || state.last_shape != shape;
                if should_write {
                    psg.write_register(13, shape);
                    state.last_shape = shape;
                }
            }
        } else {
            // Software volume (0-15)
            psg.write_register((8 + channel_in_psg) as u8, output.volume & 0x0F);
        }

        // NOTE: Mixer (register 7) and Noise (register 6) are written ONCE per PSG in write_mixer_for_psg
    }

    /// Write mixer and noise registers for all 3 channels of a PSG
    /// This matches C++ behavior where mixer is modified per-channel then written once
    ///
    /// C++ flow (in SongPlayer::fillWithChannelResult, called 3x per PSG):
    /// 1. For each channel: setMixerNoiseState() - modifies mixer bits 3-5
    /// 2. For each channel: setMixerSoundState() - modifies mixer bits 0-2
    /// 3. Mixer register written ONCE after all channels processed
    /// 4. Noise register written with last channel's noise value
    fn write_mixer_for_psg(&mut self, psg_idx: usize, channel_frames: &[ChannelFrame]) {
        if psg_idx >= self.psg_bank.psg_count() {
            return;
        }

        let psg = self.psg_bank.get_chip_mut(psg_idx);

        // Start with all channels disabled (matches C++ PsgRegisters constructor: 0b111111)
        // Note: C++ uses 0x3F (0b00111111), NOT 0xFF!
        let mut mixer: u8 = 0x3F;
        let mut noise_value: u8 = 0;

        // Process all 3 channels - each modifies the same mixer variable
        // This matches C++ setMixerNoiseState() and setMixerSoundState()
        for (ch_in_psg, frame) in channel_frames.iter().take(3).enumerate() {
            let output = &frame.psg;
            // C++: psgRegistersToFill.setMixerNoiseState(targetChannel, isNoise);
            // C++: if (open) mixer &= ~(1 << bitRank); else mixer |= (1 << bitRank);
            if output.noise > 0 {
                // Noise enabled: clear bit (0 = enabled)
                mixer &= !(1 << (ch_in_psg + 3));
                // Save noise value - last channel with noise wins
                noise_value = output.noise;
            } else {
                // Noise disabled: set bit (1 = disabled)
                mixer |= 1 << (ch_in_psg + 3);
            }

            // C++: psgRegistersToFill.setMixerSoundState(targetChannel, isSound);
            // C++: if (open) mixer &= ~(1 << bitRank); else mixer |= (1 << bitRank);
            if output.sound_open {
                // Tone enabled: clear bit (0 = enabled)
                mixer &= !(1 << ch_in_psg);
            } else {
                // Tone disabled: set bit (1 = disabled)
                mixer |= 1 << ch_in_psg;
            }
        }

        // Write noise period (register 6) BEFORE mixer
        // C++: if (isNoise) psgRegistersToFill.setNoise(noise);
        if noise_value > 0 {
            psg.write_register(6, noise_value & 0x1F);
        }

        // Write mixer register (register 7) ONCE for all 3 channels
        psg.write_register(7, mixer);
    }

    fn loop_bounds(&self, subsong_index: usize, position_count: usize) -> (usize, usize) {
        if position_count == 0 {
            return (0, 0);
        }

        let subsong = &self.song.subsongs[subsong_index];
        let loop_start = subsong
            .loop_start_position
            .min(position_count.saturating_sub(1));

        let past_end = subsong.end_position.saturating_add(1).min(position_count);
        let past_end_position = past_end.max(loop_start.saturating_add(1));

        (loop_start, past_end_position)
    }

    fn advance_song(&mut self, loop_start: usize, past_end_position: usize) {
        self.current_tick += 1;

        if self.current_tick >= self.current_speed {
            self.current_tick = 0;
            self.current_line += 1;

            let pattern_height = self.get_pattern_height_internal();
            if self.current_line >= pattern_height {
                self.current_line = 0;
                self.current_position += 1;

                if past_end_position > 0 && self.current_position >= past_end_position {
                    self.current_position = loop_start;
                }
            }
        }
    }

    fn determine_speed_for_location(&self, position: usize, line: usize) -> u8 {
        let subsong = &self.song.subsongs[self.subsong_index];
        if subsong.positions.is_empty() {
            return subsong.initial_speed;
        }

        let mut speed = subsong.initial_speed;
        let target_position = position.min(subsong.positions.len() - 1);

        for pos_idx in 0..=target_position {
            let height = subsong.positions[pos_idx].height.max(1);
            let target_line = if pos_idx == target_position {
                line.min(height - 1)
            } else {
                height - 1
            };

            if let Some(value) = self.read_special_track_value_internal(
                self.subsong_index,
                pos_idx,
                target_line,
                true,
            ) && value > 0
            {
                speed = value.clamp(1, u8::MAX as i32) as u8;
            }
        }

        speed
    }

    fn apply_speed_track(&mut self, subsong_index: usize) {
        if let Some(value) = self.read_special_track_value(subsong_index, true) {
            let clamped = value.clamp(1, u8::MAX as i32) as u8;
            self.current_speed = clamped;
        }
    }

    fn read_special_track_value(&self, subsong_index: usize, speed: bool) -> Option<i32> {
        self.read_special_track_value_internal(
            subsong_index,
            self.current_position,
            self.current_line,
            speed,
        )
    }

    fn read_special_track_value_internal(
        &self,
        subsong_index: usize,
        position_index: usize,
        line: usize,
        speed: bool,
    ) -> Option<i32> {
        let subsong = &self.song.subsongs[subsong_index];

        if position_index >= subsong.positions.len() {
            return None;
        }

        let position = &subsong.positions[position_index];
        if position.pattern_index >= subsong.patterns.len() {
            return None;
        }

        let pattern = &subsong.patterns[position.pattern_index];
        let track_index = if speed {
            pattern.speed_track_index
        } else {
            pattern.event_track_index
        };

        let tracks = if speed {
            &subsong.speed_tracks
        } else {
            &subsong.event_tracks
        };

        let track = tracks.get(&track_index)?;
        track.latest_value(line)
    }

    fn resolve_cell(
        subsong: &crate::format::Subsong,
        position_idx: usize,
        line_idx: usize,
        channel_idx: usize,
    ) -> (Option<&crate::format::Cell>, i8) {
        let mut transposition = 0;
        if let Some(position) = subsong.positions.get(position_idx) {
            if let Some(value) = position.transpositions.get(channel_idx) {
                transposition = *value;
            }

            if let Some(pattern) = subsong.patterns.get(position.pattern_index)
                && let Some(track_index) = pattern.track_indexes.get(channel_idx)
                && let Some(track) = subsong.tracks.get(track_index)
            {
                let cell = track.cells.iter().find(|c| c.index == line_idx);
                return (cell, transposition);
            }
        }

        (None, transposition)
    }

    fn trigger_event_sample(&mut self, instrument_index: usize) {
        let subsong = &self.song.subsongs[self.subsong_index];
        if subsong.digi_channel >= self.sample_voices.len() {
            return;
        }

        let instrument = match self.song.instruments.get(instrument_index) {
            Some(instr) => instr,
            None => return,
        };

        if instrument.instrument_type != InstrumentType::Digi {
            return;
        }

        let sample = match &instrument.sample {
            Some(s) => s,
            None => return,
        };

        if sample.data.is_empty() {
            return;
        }

        let channel = subsong.digi_channel;
        let psg_idx = channel / 3;
        let Some(psg) = subsong.psgs.get(psg_idx) else {
            return;
        };

        let pitch_hz = note_frequency(psg.reference_frequency, sample.digidrum_note);
        if pitch_hz <= 0.0 {
            return;
        }

        let params = SamplePlaybackParams {
            data: Arc::clone(&sample.data),
            loop_start: sample.loop_start_index,
            loop_end: sample.end_index,
            looping: sample.is_looping,
            sample_frequency_hz: sample.frequency_hz,
            pitch_hz,
            amplification: sample.amplification_ratio,
            volume: 15,
            sample_player_frequency_hz: psg.sample_player_frequency as f32,
            reference_frequency_hz: psg.reference_frequency,
            play_from_start: true,
            high_priority: true,
        };

        if let Some(voice) = self.sample_voices.get_mut(channel) {
            voice.apply_command(&SampleCommand::Play(params), self.output_sample_rate);
        }
    }

    /// Get debug information about channel states
    ///
    /// Returns (note, period, volume) for each channel
    pub fn debug_channel_states(&self) -> Vec<(u8, u16, u8)> {
        // This would need access to channel player internals
        // For now, return dummy data
        vec![(0, 0, 0); self.channel_players.len()]
    }
}

#[derive(Default, Clone)]
struct SampleVoiceMixer {
    active: Option<ActiveSample>,
}

impl SampleVoiceMixer {
    fn apply_command(&mut self, command: &SampleCommand, output_sample_rate: f32) {
        match command {
            SampleCommand::None => {}
            SampleCommand::Stop => self.active = None,
            SampleCommand::Play(params) => {
                if params.pitch_hz <= 0.0
                    || params.reference_frequency_hz <= 0.0
                    || params.sample_player_frequency_hz <= 0.0
                    || params.data.is_empty()
                {
                    self.active = None;
                    return;
                }

                let mut step = (params.sample_player_frequency_hz / output_sample_rate)
                    * (params.pitch_hz / params.reference_frequency_hz);
                if !step.is_finite() || step <= 0.0 {
                    step = 0.0;
                }

                if step == 0.0 {
                    self.active = None;
                    return;
                }

                if let Some(active) = self.active.as_ref()
                    && active.high_priority
                    && !params.high_priority
                {
                    return;
                }

                if let Some(active) = self.active.as_mut() {
                    active.update_from_params(params, step);
                    if params.play_from_start {
                        active.position = params.loop_start as f32;
                    }
                } else {
                    self.active = Some(ActiveSample::new(params, step));
                }
            }
        }
    }

    fn mix_into(&mut self, segment: &mut [f32]) {
        if self.active.is_none() {
            return;
        }

        let mut stop = false;
        {
            let active = self.active.as_mut().unwrap();
            for sample in segment.iter_mut() {
                if active.data.is_empty() {
                    stop = true;
                    break;
                }

                let idx = active.position as usize;
                if idx >= active.loop_end || idx >= active.data.len() {
                    if active.looping {
                        active.position = active.loop_start as f32;
                        continue;
                    } else {
                        stop = true;
                        break;
                    }
                }

                *sample += active.data[idx] * active.gain;
                active.advance_position();
                if active.step == 0.0 {
                    stop = true;
                    break;
                }
            }
        }

        if stop {
            self.active = None;
        }
    }
}

#[cfg(all(test, feature = "extended-tests"))]
fn frames_to_registers(psg_idx: usize, frames: &[ChannelFrame], prev: &mut [u8; 16]) -> [u8; 16] {
    let base_channel = psg_idx * 3;
    let mut regs = *prev;
    let mut mixer: u8 = 0x3F;
    let mut noise_value: Option<u8> = None;

    for offset in 0..3 {
        let channel_idx = base_channel + offset;
        if let Some(frame) = frames.get(channel_idx) {
            let output = &frame.psg;
            regs[offset * 2] = (output.software_period & 0xFF) as u8;
            regs[offset * 2 + 1] = ((output.software_period >> 8) & 0x0F) as u8;

            if output.volume == 16 {
                regs[8 + offset] = 0x10;
                regs[11] = (output.hardware_period & 0xFF) as u8;
                regs[12] = ((output.hardware_period >> 8) & 0xFF) as u8;
                regs[13] = output.hardware_envelope & 0x0F;
            } else {
                regs[8 + offset] = output.volume & 0x0F;
            }

            if output.noise > 0 {
                mixer &= !(1 << (offset + 3));
                noise_value = Some(output.noise & 0x1F);
            } else {
                mixer |= 1 << (offset + 3);
            }

            if output.sound_open {
                mixer &= !(1 << offset);
            } else {
                mixer |= 1 << offset;
            }
        }
    }

    if let Some(noise) = noise_value {
        regs[6] = noise;
    }
    regs[7] = mixer;
    *prev = regs;
    regs
}

#[cfg(all(test, feature = "extended-tests"))]
mod tests {
    use super::*;
    use crate::parser::load_aks;
    use std::path::PathBuf;
    use ym_replayer::parser::ym6::Ym6Parser;

    fn normalize_ym_registers(frame: &[u8], prev: &mut [u8; 16]) -> [u8; 16] {
        let mut regs = [0u8; 16];
        let len = frame.len().min(16);
        regs[..len].copy_from_slice(&frame[..len]);
        if regs[13] == 0xFF {
            regs[13] = prev[13];
        }
        *prev = regs;
        regs
    }

    #[test]
    fn doclands_first_tick_contains_hardware_channel() {
        use std::path::PathBuf;

        let path: PathBuf = [
            env!("CARGO_MANIFEST_DIR"),
            "..",
            "..",
            "examples",
            "arkos",
            "Doclands - Pong Cracktro (YM).aks",
        ]
        .iter()
        .collect();
        let data = std::fs::read(path).expect("load Doclands .aks");
        let song = load_aks(&data).expect("parse Doclands");
        let mut player = ArkosPlayer::new(song, 0).expect("player init");

        let frames = player.capture_tick_frames();
        assert!(
            frames.iter().any(|frame| frame.psg.volume == 16),
            "expected at least one hardware channel on first tick"
        );
        assert!(
            frames.iter().any(|frame| frame.psg.noise > 0),
            "expected noise activity on first tick, got {:?}",
            frames
                .iter()
                .map(|frame| frame.psg.noise)
                .collect::<Vec<_>>()
        );
    }

    fn data_path(file: &str) -> PathBuf {
        [
            env!("CARGO_MANIFEST_DIR"),
            "..",
            "..",
            "examples",
            "arkos",
            file,
        ]
        .iter()
        .collect()
    }

    #[test]
    fn at2_first_tick_is_silent() {
        let aks_path = data_path("Excellence in Art 2018 - Just add cream.aks");
        let song_data = std::fs::read(aks_path).expect("read AT2 AKS");
        let song = load_aks(&song_data).expect("parse AT2 AKS");
        let mut player = ArkosPlayer::new(song, 0).expect("init AT2 player");

        assert!(
            !player.song.subsongs[player.subsong_index]
                .positions
                .is_empty(),
            "AT2 subsong should have synthesized positions"
        );
        let subsong = &player.song.subsongs[player.subsong_index];
        let (cell, _) = ArkosPlayer::resolve_cell(subsong, 0, 0, 0);
        let cell = cell.expect("expected a cell at first line/channel for AT2 pattern 0");
        assert!(
            cell.instrument_present && cell.instrument == usize::MAX,
            "legacy cells with instrument 0 should be marked as RST sentinel"
        );
        let frames = player.capture_tick_frames();
        assert!(
            frames.iter().all(|frame| frame.psg.volume == 0),
            "expected volume 0 on first tick before the buzzer is primed, got {:?}",
            frames
        );
    }

    #[test]
    #[ignore]
    fn doclands_matches_reference_ym() {
        let ym_path = data_path("Doclands - Pong Cracktro (YM).ym");
        let ym_data = std::fs::read(ym_path).expect("read reference YM");
        let parser = Ym6Parser;
        let (ym_frames, header, _, _) = parser.parse_full(&ym_data).expect("parse YM");

        let aks_path = data_path("Doclands - Pong Cracktro (YM).aks");
        let song_data = std::fs::read(aks_path).expect("read Doclands AKS");
        let song = load_aks(&song_data).expect("parse Doclands AKS");
        let mut player = ArkosPlayer::new(song, 0).expect("init player");

        let frame_limit = ym_frames.len();
        let mut prev_expected = [0u8; 16];
        let mut prev_actual = [0u8; 16];
        prev_actual[13] = 8;
        for (idx, frame) in ym_frames.iter().take(frame_limit).enumerate() {
            let expected = normalize_ym_registers(frame, &mut prev_expected);
            let frames = player.capture_tick_frames();
            let actual = frames_to_registers(0, &frames, &mut prev_actual);
            if idx < 8 {
                println!(
                    "Frame {idx}: expected {:?} actual {:?}",
                    &expected[..14],
                    &actual[..14]
                );
            }
            if expected[..14] != actual[..14] {
                println!(
                    "Context at position {} line {}:",
                    player.current_position, player.current_line
                );
                if let Some(subsong) = player.song.subsongs.get(player.subsong_index)
                    && let Some(pos) = subsong.positions.get(player.current_position)
                    && let Some(pattern) = subsong.patterns.get(pos.pattern_index)
                {
                    println!("  pattern track indexes {:?}", pattern.track_indexes);
                }
                for ch in 0..player.channel_players.len() {
                    if let Some(subsong) = player.song.subsongs.get(player.subsong_index)
                        && let Some(pos) = subsong.positions.get(player.current_position)
                        && let Some(pattern) = subsong.patterns.get(pos.pattern_index)
                        && let Some(track_idx) = pattern.track_indexes.get(ch)
                    {
                        println!("  ch{ch} uses track {}", track_idx);
                        if let Some(track) = subsong.tracks.get(track_idx) {
                            let indices: Vec<_> = track.cells.iter().map(|c| c.index).collect();
                            println!("    track cells {:?}", indices);
                        }
                    }
                    let ctx = player
                        .effect_context
                        .line_context(player.current_position, ch, player.current_line)
                        .cloned();
                    println!("  ch{ch} ctx {:?}", ctx);
                }
                println!("Frame {} mismatch:", idx);
                println!("  current speed {}", player.current_speed);
                println!("  Expected regs: {:?}", &expected[..14]);
                println!("  Actual regs:   {:?}", &actual[..14]);
                for (channel, frame) in frames.iter().enumerate().take(3) {
                    let out = &frame.psg;
                    println!(
                        "  Ch{}: vol {:>2} noise {:>2} sound_open {} period {:>4} hw_period {:>4} env {:>2} sample {:?}",
                        channel,
                        out.volume,
                        out.noise,
                        out.sound_open,
                        out.software_period,
                        out.hardware_period,
                        out.hardware_envelope,
                        frame.sample
                    );
                    let state = player.channel_players[channel].debug_state();
                    println!("    state: {:?}", state);
                }
                panic!(
                    "Register mismatch at frame {} (of {})",
                    idx, header.frame_count
                );
            }
        }
    }

    #[test]
    #[ignore]
    fn lop_ears_matches_reference_ym() {
        let ym_path = data_path("Andy Severn - Lop Ears.ym");
        let ym_data = std::fs::read(ym_path).expect("read Lop Ears YM");
        let parser = Ym6Parser;
        let (ym_frames, header, _, _) = parser.parse_full(&ym_data).expect("parse Lop Ears YM");

        let aks_path = data_path("Andy Severn - Lop Ears.aks");
        let song_data = std::fs::read(aks_path).expect("read Lop Ears AKS");
        let song = load_aks(&song_data).expect("parse Lop Ears AKS");
        #[cfg(test)]
        {
            let inst = &song.instruments[1];
            let links: Vec<_> = inst
                .cells
                .iter()
                .enumerate()
                .map(|(idx, cell)| (idx, format!("{:?}", cell.link)))
                .collect();
            println!("lop ears instrument1 links {:?}", links);
            if let Some(track) = song.subsongs[0].tracks.get(&8) {
                println!(
                    "lop ears track8 cells {:?}",
                    track.cells.iter().map(|c| c.index).collect::<Vec<_>>()
                );
            }
        }
        let mut player = ArkosPlayer::new(song, 0).expect("init Lop Ears player");

        let frame_limit = ym_frames.len();
        let mut prev_expected = [0u8; 16];
        let mut prev_actual = [0u8; 16];
        prev_actual[13] = 8;
        for (idx, frame) in ym_frames.iter().take(frame_limit).enumerate() {
            let expected = normalize_ym_registers(frame, &mut prev_expected);
            let frames = player.capture_tick_frames();
            let actual = frames_to_registers(0, &frames, &mut prev_actual);
            if idx < 8 {
                continue;
            }
            if expected[..14] != actual[..14] {
                println!(
                    "Context at position {} line {}:",
                    player.current_position, player.current_line
                );
                for ch in 0..player.channel_players.len() {
                    if let Some(subsong) = player.song.subsongs.get(player.subsong_index)
                        && let Some(pos) = subsong.positions.get(player.current_position)
                        && let Some(pattern) = subsong.patterns.get(pos.pattern_index)
                        && let Some(track_idx) = pattern.track_indexes.get(ch)
                    {
                        println!("  ch{ch} uses track {}", track_idx);
                    }
                    let ctx = player
                        .effect_context
                        .line_context(player.current_position, ch, player.current_line)
                        .cloned();
                    println!("  ch{ch} ctx {:?}", ctx);
                }
                println!("Frame {} mismatch:", idx);
                println!("  current speed {}", player.current_speed);
                println!("  Expected regs: {:?}", &expected[..14]);
                println!("  Actual regs:   {:?}", &actual[..14]);
                for (channel, frame) in frames.iter().enumerate().take(3) {
                    let out = &frame.psg;
                    println!(
                        "  Ch{}: vol {:>2} noise {:>2} sound_open {} period {:>4} hw_period {:>4} env {:>2} sample {:?}",
                        channel,
                        out.volume,
                        out.noise,
                        out.sound_open,
                        out.software_period,
                        out.hardware_period,
                        out.hardware_envelope,
                        frame.sample
                    );
                }
                panic!(
                    "Lop Ears mismatch at frame {} (of {})",
                    idx, header.frame_count
                );
            }
        }
    }

    #[test]
    #[ignore]
    fn debug_doclands_first_cell() {
        let aks_path = data_path("Doclands - Pong Cracktro (YM).aks");
        let song_data = std::fs::read(aks_path).expect("read Doclands AKS");
        let song = load_aks(&song_data).expect("parse Doclands AKS");
        let subsong = &song.subsongs[0];
        for channel_idx in 0..3 {
            if let Some((track_index, cell)) = subsong
                .positions
                .first()
                .and_then(|position| subsong.patterns.get(position.pattern_index))
                .and_then(|pattern| {
                    pattern
                        .track_indexes
                        .get(channel_idx)
                        .and_then(|track_index| {
                            subsong
                                .tracks
                                .get(track_index)
                                .map(|track| (track_index, track))
                        })
                })
                .and_then(|(track_index, track)| {
                    track
                        .cells
                        .iter()
                        .find(|c| c.index == 0)
                        .map(|cell| (*track_index, cell))
                })
            {
                println!(
                    "Channel {channel_idx}: track {track_index} note {} instrument {} effects {:?}",
                    cell.note, cell.instrument, cell.effects
                );
                if cell.instrument < song.instruments.len() {
                    let instrument = &song.instruments[cell.instrument];
                    if !instrument.cells.is_empty() {
                        let inst_cell = &instrument.cells[0];
                        println!(
                            "  instrument cell volume {} noise {} link {:?} prim_arp_note {} prim_arp_oct {} prim_pitch {} sec_arp_note {} sec_arp_oct {} sec_pitch {} forced_sw {} forced_hw {}",
                            inst_cell.volume,
                            inst_cell.noise,
                            inst_cell.link,
                            inst_cell.primary_arpeggio_note_in_octave,
                            inst_cell.primary_arpeggio_octave,
                            inst_cell.primary_pitch,
                            inst_cell.secondary_arpeggio_note_in_octave,
                            inst_cell.secondary_arpeggio_octave,
                            inst_cell.secondary_pitch,
                            inst_cell.primary_period,
                            inst_cell.secondary_period
                        );
                    }
                }
            } else {
                println!("Channel {channel_idx}: No cell at position 0, line 0");
            }
        }
    }

    #[test]
    #[ignore]
    fn debug_doclands_pattern_tracks() {
        let aks_path = data_path("Doclands - Pong Cracktro (YM).aks");
        let song_data = std::fs::read(aks_path).expect("read Doclands AKS");
        let song = load_aks(&song_data).expect("parse Doclands AKS");
        let subsong = &song.subsongs[0];
        for (pattern_idx, pattern) in subsong.patterns.iter().enumerate().take(4) {
            println!("Pattern {pattern_idx}: {:?}", pattern.track_indexes);
        }
        for (pos_idx, position) in subsong.positions.iter().enumerate().take(4) {
            println!(
                "Position {pos_idx}: pattern {} height {}",
                position.pattern_index, position.height
            );
        }
    }

    #[test]
    #[ignore]
    fn debug_doclands_track_cells() {
        let aks_path = data_path("Doclands - Pong Cracktro (YM).aks");
        let song_data = std::fs::read(aks_path).expect("read Doclands AKS");
        let song = load_aks(&song_data).expect("parse Doclands AKS");
        let subsong = &song.subsongs[0];
        for (track_idx, track) in subsong.tracks.iter() {
            println!("Track {track_idx}");
            for cell in track.cells.iter().take(8) {
                println!(
                    "  line {:>2}: note {:>3} instrument {} effects {:?}",
                    cell.index, cell.note, cell.instrument, cell.effects
                );
            }
        }
    }

    #[test]
    #[ignore]
    fn debug_doclands_first_frames_registers() {
        let ym_path = data_path("Doclands - Pong Cracktro (YM).ym");
        let ym_data = std::fs::read(ym_path).expect("read reference YM");
        let parser = Ym6Parser;
        let (ym_frames, _, _, _) = parser.parse_full(&ym_data).expect("parse YM");

        let aks_path = data_path("Doclands - Pong Cracktro (YM).aks");
        let song_data = std::fs::read(aks_path).expect("read Doclands AKS");
        let song = load_aks(&song_data).expect("parse Doclands AKS");
        let mut player = ArkosPlayer::new(song, 0).expect("init doc player");

        let mut prev_expected = [0u8; 16];
        let mut prev_actual = [0u8; 16];
        prev_actual[13] = 8;

        for (idx, frame) in ym_frames.iter().take(16).enumerate() {
            let expected = normalize_ym_registers(frame, &mut prev_expected);
            let frames = player.capture_tick_frames();
            let actual = frames_to_registers(0, &frames, &mut prev_actual);
            println!(
                "#{idx:02} pos {} line {} speed {} exp {:?} act {:?}",
                player.current_position,
                player.current_line,
                player.current_speed,
                &expected[..14],
                &actual[..14]
            );
            for (ch, frame) in frames.iter().enumerate().take(3) {
                let out = &frame.psg;
                let state = player.channel_players[ch].debug_state();
                println!(
                    "   ch{} vol {:>2} noise {:>2} period {:>4} inst {:?}",
                    ch, out.volume, out.noise, out.software_period, state
                );
            }
        }
    }

    #[test]
    #[ignore]
    fn debug_doclands_speed_tracks() {
        let aks_path = data_path("Doclands - Pong Cracktro (YM).aks");
        let song_data = std::fs::read(aks_path).expect("read Doclands AKS");
        let song = load_aks(&song_data).expect("parse Doclands AKS");
        let subsong = &song.subsongs[0];
        println!(
            "speed tracks available: {:?}",
            subsong.speed_tracks.keys().collect::<Vec<_>>()
        );
        let pattern = &subsong.patterns[subsong.positions[0].pattern_index];
        println!("pattern0 speed idx {}", pattern.speed_track_index);
        if let Some(track) = subsong.speed_tracks.get(&pattern.speed_track_index) {
            println!("track cells {:?}", track.cells);
        } else {
            println!("track not found!");
        }
    }

    #[test]
    #[ignore]
    fn at2_matches_reference_ym() {
        let ym_path = data_path("Excellence in Art 2018 - Just add cream.ym");
        let ym_data = std::fs::read(ym_path).expect("read AT2 YM");
        let parser = Ym6Parser;
        let (ym_frames, header, _, _) = parser.parse_full(&ym_data).expect("parse AT2 YM");

        let aks_path = data_path("Excellence in Art 2018 - Just add cream.aks");
        let song_data = std::fs::read(aks_path).expect("read AT2 AKS");
        let song = load_aks(&song_data).expect("parse AT2 AKS");
        let mut player = ArkosPlayer::new(song, 0).expect("init AT2 player");

        let frame_limit = ym_frames.len();
        let mut prev_expected = [0u8; 16];
        let mut prev_actual = [0u8; 16];
        prev_actual[13] = 8;
        for (idx, frame) in ym_frames.iter().take(frame_limit).enumerate() {
            let expected = normalize_ym_registers(frame, &mut prev_expected);
            let frames = player.capture_tick_frames();
            let actual = frames_to_registers(0, &frames, &mut prev_actual);
            if expected[..14] != actual[..14] {
                println!(
                    "AT2 mismatch at frame {} position {} line {}",
                    idx, player.current_position, player.current_line
                );
                if let Some(subsong) = player.song.subsongs.get(player.subsong_index)
                    && let Some(pos) = subsong.positions.get(player.current_position)
                {
                    if let Some(pattern) = subsong.patterns.get(pos.pattern_index) {
                        println!(
                            "  pattern {} track idx {:?} speed {} event {}",
                            pattern.index,
                            pattern.track_indexes,
                            pattern.speed_track_index,
                            pattern.event_track_index
                        );
                    }
                    println!("  position transpositions {:?}", pos.transpositions);
                }
                for ch in 0..player.channel_players.len().min(3) {
                    if let Some(subsong) = player.song.subsongs.get(player.subsong_index)
                        && let Some(pos) = subsong.positions.get(player.current_position)
                        && let Some(pattern) = subsong.patterns.get(pos.pattern_index)
                        && let Some(track_idx) = pattern.track_indexes.get(ch)
                    {
                        println!("  ch{ch} uses track {}", track_idx);
                    }
                    let ctx = player
                        .effect_context
                        .line_context(player.current_position, ch, player.current_line)
                        .cloned();
                    println!("  ch{ch} ctx {:?}", ctx);
                }
                println!("Frame {} mismatch:", idx);
                println!("  current speed {}", player.current_speed);
                println!("  Expected regs: {:?}", &expected[..14]);
                println!("  Actual regs:   {:?}", &actual[..14]);
                for (channel, frame) in frames.iter().enumerate().take(3) {
                    let out = &frame.psg;
                    println!(
                        "  Ch{}: vol {:>2} noise {:>2} sound_open {} period {:>4} hw_period {:>4} env {:>2} sample {:?}",
                        channel,
                        out.volume,
                        out.noise,
                        out.sound_open,
                        out.software_period,
                        out.hardware_period,
                        out.hardware_envelope,
                        frame.sample
                    );
                    let state = player.channel_players[channel].debug_state();
                    println!("    state: {:?}", state);
                }
                panic!(
                    "AT2 Register mismatch at frame {} (of {})",
                    idx, header.frame_count
                );
            }
        }
    }
}

#[derive(Clone)]
struct ActiveSample {
    data: Arc<Vec<f32>>,
    position: f32,
    loop_start: usize,
    loop_end: usize,
    looping: bool,
    step: f32,
    gain: f32,
    high_priority: bool,
}

impl ActiveSample {
    fn new(params: &SamplePlaybackParams, step: f32) -> Self {
        let mut loop_end = params.loop_end.saturating_add(1);
        loop_end = loop_end.min(params.data.len());
        if loop_end == 0 {
            loop_end = params.data.len();
        }
        let mut loop_start = params.loop_start.min(loop_end.saturating_sub(1));
        if loop_start >= loop_end {
            loop_start = 0;
        }

        let mut instance = Self {
            data: Arc::clone(&params.data),
            position: loop_start as f32,
            loop_start,
            loop_end,
            looping: params.looping,
            step,
            gain: Self::compute_gain(params),
            high_priority: params.high_priority,
        };
        if !params.play_from_start {
            instance.position = loop_start as f32;
        }
        instance
    }

    fn update_from_params(&mut self, params: &SamplePlaybackParams, step: f32) {
        self.data = Arc::clone(&params.data);
        let mut loop_end = params.loop_end.saturating_add(1).min(self.data.len());
        if loop_end == 0 {
            loop_end = self.data.len();
        }
        let mut loop_start = params.loop_start.min(loop_end.saturating_sub(1));
        if loop_start >= loop_end {
            loop_start = 0;
        }
        self.loop_start = loop_start;
        self.loop_end = loop_end;
        self.looping = params.looping;
        self.step = step;
        self.gain = Self::compute_gain(params);
        self.high_priority = params.high_priority;
    }

    fn compute_gain(params: &SamplePlaybackParams) -> f32 {
        let volume = (params.volume as f32 / 15.0).clamp(0.0, 1.0);
        volume * params.amplification
    }

    fn advance_position(&mut self) {
        let mut next_position = self.position + self.step;
        if next_position.is_nan() || !next_position.is_finite() {
            self.step = 0.0;
            return;
        }

        while next_position as usize >= self.loop_end {
            if self.looping {
                let overflow = next_position - self.loop_end as f32;
                next_position = self.loop_start as f32 + overflow;
            } else {
                self.step = 0.0;
                self.step = 0.0;
                break;
            }
        }

        self.position = next_position;
    }
}

fn note_frequency(reference_frequency: f32, note: i32) -> f32 {
    if note < 0 {
        return 0.0;
    }

    const START_OCTAVE: i32 = -3;
    const NOTES_IN_OCTAVE: i32 = 12;

    let octave = (note / NOTES_IN_OCTAVE) + START_OCTAVE;
    let note_in_octave = (note % NOTES_IN_OCTAVE) + 1;

    ((reference_frequency as f64)
        * 2.0_f64.powf(octave as f64 + ((note_in_octave as f64 - 10.0) / 12.0))) as f32
}

#[derive(Clone, Copy)]
struct HardwareEnvelopeState {
    last_shape: u8,
}

impl Default for HardwareEnvelopeState {
    fn default() -> Self {
        Self { last_shape: 0xFF }
    }
}
