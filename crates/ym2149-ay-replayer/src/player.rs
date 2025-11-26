//! High-level AY song player (Z80 + YM2149 bridge).

use iz80::{Cpu, Machine, Reg8, Reg16};
use std::mem;

use crate::error::{AyError, Result};
use crate::format::{AyFile, AyPoints, AySong};
use crate::machine::AyMachine;
use ym2149_common::{ChiptunePlayer, PlaybackMetadata, PlaybackState};

const SAMPLE_RATE: u32 = 44_100;
const FRAME_RATE_HZ: f32 = 50.0;
const RETURN_ADDRESS: u16 = 0x0000;
const MAX_INSTRUCTIONS_PER_CALL: usize = 250_000;
const ZX_CPU_CLOCK_HZ: f64 = 3_500_000.0;
const CPC_CPU_CLOCK_HZ: f64 = 4_000_000.0;
/// User-facing message when CPC AY playback is attempted.
pub const CPC_UNSUPPORTED_MSG: &str =
    "CPC AY songs currently require full CPC firmware emulation, which is not supported";

/// Backwards compatibility alias for `PlaybackState`.
#[deprecated(since = "0.7.0", note = "Use `PlaybackState` from `ym2149_common` instead")]
pub type AyPlaybackState = PlaybackState;

/// Runtime metadata about the currently loaded song.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AyMetadata {
    /// Human-readable name extracted from the song entry.
    pub song_name: String,
    /// File-level author string.
    pub author: String,
    /// File-level misc/comment string.
    pub misc: String,
    /// Song index inside the container.
    pub song_index: usize,
    /// Total number of songs in the file.
    pub song_count: usize,
    /// Optional declared frame count.
    pub frame_count: Option<usize>,
    /// Optional duration in seconds.
    pub duration_seconds: Option<f32>,
    /// File format version.
    pub file_version: u16,
    /// Requested player version.
    pub player_version: u8,
}

impl PlaybackMetadata for AyMetadata {
    fn title(&self) -> &str {
        &self.song_name
    }

    fn author(&self) -> &str {
        &self.author
    }

    fn comments(&self) -> &str {
        &self.misc
    }

    fn format(&self) -> &str {
        "AY"
    }

    fn frame_count(&self) -> Option<usize> {
        self.frame_count
    }

    fn frame_rate(&self) -> u32 {
        50
    }

    fn duration_seconds(&self) -> Option<f32> {
        self.duration_seconds
    }
}

impl AyMetadata {
    /// Convenience helper for user-facing descriptions.
    pub fn description(&self) -> String {
        format!(
            "{} — {} ({}/{})",
            self.song_name,
            self.author,
            self.song_index + 1,
            self.song_count
        )
    }
}

/// High-level AY song player.
pub struct AyPlayer {
    song: AySong,
    metadata: AyMetadata,
    points: AyPoints,
    init_address: u16,
    interrupt_address: u16,
    machine: AyMachine,
    cpu: Cpu,
    samples_per_frame: usize,
    sample_cache: Vec<f32>,
    cache_pos: usize,
    cache_len: usize,
    frame_counter: usize,
    max_frames: Option<usize>,
    state: PlaybackState,
    init_executed: bool,
    sample_period: f64,
}

impl AyPlayer {
    /// Create a player for the selected song index.
    pub fn new(file: AyFile, song_index: usize) -> Result<Self> {
        if song_index >= file.songs.len() {
            return Err(AyError::InvalidData {
                msg: format!(
                    "Song index {song_index} out of range ({} available)",
                    file.songs.len()
                ),
            });
        }

        let header = file.header;
        let song = file.songs[song_index].clone();
        let points = song
            .data
            .points
            .clone()
            .ok_or_else(|| AyError::InvalidData {
                msg: "AY song is missing points data".to_string(),
            })?;
        let init_address = resolve_init_address(&song, &points)?;
        let interrupt_address = if points.interrupt != 0 {
            points.interrupt
        } else {
            init_address
        };

        let samples_per_frame = (SAMPLE_RATE as f32 / FRAME_RATE_HZ).round() as usize;
        let metadata = build_metadata(&header, song_index, file.songs.len(), &song);
        let mut player = Self {
            song,
            metadata,
            points,
            init_address,
            interrupt_address,
            machine: AyMachine::new(SAMPLE_RATE),
            cpu: Cpu::new(),
            samples_per_frame,
            sample_cache: Vec::with_capacity(samples_per_frame),
            cache_pos: 0,
            cache_len: 0,
            frame_counter: 0,
            max_frames: frame_limit(&file.songs[song_index]),
            state: PlaybackState::Stopped,
            init_executed: false,
            sample_period: 1.0 / SAMPLE_RATE as f64,
        };

        player.reset_runtime()?;
        Ok(player)
    }

    /// Helper that parses bytes and builds both metadata + player.
    pub fn load_from_bytes(data: &[u8], song_index: usize) -> Result<(Self, AyMetadata)> {
        let file = crate::parser::load_ay(data)?;
        if song_index >= file.songs.len() {
            return Err(AyError::InvalidData {
                msg: format!(
                    "Song index {song_index} out of range ({} available)",
                    file.songs.len()
                ),
            });
        }
        let metadata_stub = build_metadata(
            &file.header,
            song_index,
            file.songs.len(),
            &file.songs[song_index],
        );
        let player = AyPlayer::new(file, song_index)?;
        Ok((player, metadata_stub))
    }

    /// Access metadata.
    pub fn metadata(&self) -> &AyMetadata {
        &self.metadata
    }

    /// Return playback state.
    pub fn playback_state(&self) -> PlaybackState {
        self.state
    }

    /// Begin playback or resume from pause.
    pub fn play(&mut self) -> Result<()> {
        match self.state {
            PlaybackState::Playing => {}
            PlaybackState::Paused => self.state = PlaybackState::Playing,
            PlaybackState::Stopped => {
                self.reset_runtime()?;
                self.state = PlaybackState::Playing;
            }
        }
        Ok(())
    }

    /// Pause playback (keep current state).
    pub fn pause(&mut self) {
        if self.state == PlaybackState::Playing {
            self.state = PlaybackState::Paused;
        }
    }

    /// Stop playback and reset to the beginning.
    pub fn stop(&mut self) -> Result<()> {
        if self.state != PlaybackState::Stopped {
            self.state = PlaybackState::Stopped;
            self.reset_runtime()?;
        }
        Ok(())
    }

    /// Generate mono samples into a freshly allocated buffer.
    pub fn generate_samples(&mut self, count: usize) -> Vec<f32> {
        let mut output = vec![0.0; count];
        self.generate_samples_into(&mut output);
        output
    }

    /// Generate mono samples into the provided buffer.
    pub fn generate_samples_into(&mut self, buffer: &mut [f32]) {
        let mut written = 0;
        while written < buffer.len() {
            if self.cache_pos >= self.cache_len {
                if self.state != PlaybackState::Playing {
                    buffer[written..].fill(0.0);
                    return;
                }
                if let Err(err) = self.render_frame() {
                    eprintln!("AY frame rendering error: {err}");
                    buffer[written..].fill(0.0);
                    self.state = PlaybackState::Stopped;
                    return;
                }
                if self.cache_len == 0 {
                    buffer[written..].fill(0.0);
                    return;
                }
            }

            let available = self.cache_len - self.cache_pos;
            let needed = buffer.len() - written;
            let to_copy = available.min(needed);
            buffer[written..written + to_copy]
                .copy_from_slice(&self.sample_cache[self.cache_pos..self.cache_pos + to_copy]);
            self.cache_pos += to_copy;
            written += to_copy;
        }
    }

    /// Access the underlying YM2149 chip.
    pub fn chip(&self) -> &ym2149::ym2149::Ym2149 {
        self.machine.chip()
    }

    /// Mutable access to the underlying YM2149 chip.
    pub fn chip_mut(&mut self) -> &mut ym2149::ym2149::Ym2149 {
        self.machine.chip_mut()
    }

    /// Mute/unmute a PSG channel.
    pub fn set_channel_mute(&mut self, channel: usize, mute: bool) {
        self.machine.chip_mut().set_channel_mute(channel, mute);
    }

    #[cfg(feature = "trace-ports")]
    /// Retrieve and clear the recorded port log (trace-ports feature).
    pub fn take_port_log(&mut self) -> Vec<String> {
        self.machine.take_port_log()
    }

    /// Whether the current song requires CPC firmware emulation.
    pub fn requires_cpc_firmware(&self) -> bool {
        self.machine.requires_cpc_firmware()
    }

    /// Check mute state of a PSG channel.
    pub fn is_channel_muted(&self, channel: usize) -> bool {
        self.machine.chip().is_channel_muted(channel)
    }

    /// Enable or disable ST-style color filter.
    pub fn set_color_filter(&mut self, enabled: bool) {
        self.machine.chip_mut().set_color_filter(enabled);
    }

    /// Playback position (0.0-1.0) when length is known.
    pub fn playback_position(&self) -> f32 {
        if let Some(max_frames) = self.max_frames {
            if max_frames == 0 {
                return 0.0;
            }
            (self.frame_counter.min(max_frames) as f32) / (max_frames as f32)
        } else {
            0.0
        }
    }

    /// Current frame index (0-based).
    pub fn current_frame(&self) -> usize {
        self.frame_counter
    }

    fn reset_runtime(&mut self) -> Result<()> {
        self.machine.reset_layout();
        for block in &self.song.data.blocks {
            self.machine.load_block(block);
        }
        self.cpu = Cpu::new();
        self.apply_register_presets();
        self.frame_counter = 0;
        self.cache_pos = 0;
        self.cache_len = 0;
        self.sample_cache.clear();
        self.init_executed = false;
        Ok(())
    }

    fn apply_register_presets(&mut self) {
        let preset = ((self.song.data.hi_reg as u16) << 8) | self.song.data.lo_reg as u16;
        self.cpu.registers().set16(Reg16::AF, preset);
        self.cpu.registers().set16(Reg16::BC, preset);
        self.cpu.registers().set16(Reg16::DE, preset);
        self.cpu.registers().set16(Reg16::HL, preset);
        self.cpu.registers().set16(Reg16::IX, preset);
        self.cpu.registers().set16(Reg16::IY, preset);
        self.cpu.registers().set16(Reg16::SP, self.points.stack);
        self.cpu.registers().set_pc(RETURN_ADDRESS);
        self.cpu.registers().set8(Reg8::I, 3);
    }

    fn ensure_initialized(&mut self) -> Result<()> {
        if !self.init_executed {
            self.run_subroutine(self.init_address)?;
            self.init_executed = true;
        }
        Ok(())
    }

    fn render_frame(&mut self) -> Result<()> {
        self.ensure_initialized()?;
        if self.sample_cache.len() != self.samples_per_frame {
            self.sample_cache.resize(self.samples_per_frame, 0.0);
        }
        let mut buffer = mem::take(&mut self.sample_cache);
        self.render_interrupt_stream(&mut buffer)?;
        self.sample_cache = buffer;
        self.cache_pos = 0;
        self.cache_len = self.sample_cache.len();
        self.frame_counter = self.frame_counter.saturating_add(1);
        if let Some(limit) = self.max_frames
            && self.frame_counter >= limit
        {
            self.state = PlaybackState::Stopped;
        }
        Ok(())
    }

    fn render_interrupt_stream(&mut self, buffer: &mut [f32]) -> Result<()> {
        self.fail_if_cpc()?;
        self.emulate_call(self.interrupt_address);
        let mut next_sample_time = self.sample_period;
        let mut cpu_time = 0.0f64;
        let mut idx = 0usize;
        let mut guard = MAX_INSTRUCTIONS_PER_CALL;

        while idx < buffer.len() {
            self.fail_if_cpc()?;
            while cpu_time < next_sample_time {
                self.fail_if_cpc()?;
                if self.cpu.immutable_registers().pc() == RETURN_ADDRESS {
                    cpu_time = next_sample_time;
                    break;
                }
                let before = self.cpu.cycle_count();
                self.cpu.execute_instruction(&mut self.machine);
                let after = self.cpu.cycle_count();
                let delta_cycles =
                    after
                        .checked_sub(before)
                        .ok_or_else(|| AyError::InvalidData {
                            msg: "CPU cycle counter underflowed".to_string(),
                        })? as f64;
                let cpu_clock = if self.machine.is_cpc_mode() {
                    CPC_CPU_CLOCK_HZ
                } else {
                    ZX_CPU_CLOCK_HZ
                };
                cpu_time += delta_cycles / cpu_clock;
                guard = guard.checked_sub(1).ok_or_else(|| AyError::InvalidData {
                    msg: format!(
                        "Interrupt routine at 0x{:04x} exceeded instruction budget",
                        self.interrupt_address
                    ),
                })?;
            }

            let chip = self.machine.chip_mut();
            chip.clock();
            buffer[idx] = chip.get_sample();
            idx += 1;
            next_sample_time += self.sample_period;
        }

        if self.cpu.immutable_registers().pc() != RETURN_ADDRESS {
            return Err(AyError::InvalidData {
                msg: format!(
                    "Interrupt routine at 0x{:04x} did not return before frame end",
                    self.interrupt_address
                ),
            });
        }

        Ok(())
    }

    fn run_subroutine(&mut self, entry: u16) -> Result<()> {
        self.emulate_call(entry);
        let mut guard = MAX_INSTRUCTIONS_PER_CALL;
        loop {
            self.fail_if_cpc()?;
            self.cpu.execute_instruction(&mut self.machine);
            let pc = self.cpu.immutable_registers().pc();
            if pc == RETURN_ADDRESS {
                break;
            }
            guard = guard.checked_sub(1).ok_or_else(|| AyError::InvalidData {
                msg: format!(
                    "Subroutine at 0x{entry:04x} did not return within instruction budget"
                ),
            })?;
        }
        Ok(())
    }

    fn emulate_call(&mut self, entry: u16) {
        let regs = self.cpu.registers();
        let mut sp = regs.get16(Reg16::SP);
        sp = sp.wrapping_sub(1);
        self.machine.poke(sp, (RETURN_ADDRESS >> 8) as u8);
        sp = sp.wrapping_sub(1);
        self.machine.poke(sp, RETURN_ADDRESS as u8);
        regs.set16(Reg16::SP, sp);
        regs.set_pc(entry);
    }

    fn fail_if_cpc(&self) -> Result<()> {
        if self.machine.requires_cpc_firmware() {
            Err(AyError::InvalidData {
                msg: CPC_UNSUPPORTED_MSG.to_string(),
            })
        } else {
            Ok(())
        }
    }
}

fn resolve_init_address(song: &AySong, points: &AyPoints) -> Result<u16> {
    if points.init != 0 {
        return Ok(points.init);
    }
    let block = song
        .data
        .blocks
        .first()
        .ok_or_else(|| AyError::InvalidData {
            msg: "AY song provides no memory blocks".to_string(),
        })?;
    for idx in 0..block.data.len().saturating_sub(2) {
        if block.data[idx] == 0xCD {
            let addr = u16::from_le_bytes([block.data[idx + 1], block.data[idx + 2]]);
            if addr != 0 {
                return Ok(addr);
            }
        }
    }
    Err(AyError::InvalidData {
        msg: "Unable to infer INIT address".to_string(),
    })
}

fn frame_limit(song: &AySong) -> Option<usize> {
    if song.data.song_length_50hz == 0 {
        None
    } else {
        Some(song.data.song_length_50hz as usize)
    }
}

fn build_metadata(
    header: &crate::format::AyHeader,
    song_index: usize,
    song_count: usize,
    song: &AySong,
) -> AyMetadata {
    let frame_count = frame_limit(song);
    let duration_seconds = frame_count.map(|frames| frames as f32 / FRAME_RATE_HZ);
    AyMetadata {
        song_name: song.name.clone(),
        author: header.author.clone(),
        misc: header.misc.clone(),
        song_index,
        song_count,
        frame_count,
        duration_seconds,
        file_version: header.file_version,
        player_version: header.player_version,
    }
}

// ============================================================================
// ChiptunePlayer trait implementation
// ============================================================================

impl ChiptunePlayer for AyPlayer {
    type Metadata = AyMetadata;

    fn play(&mut self) {
        let _ = AyPlayer::play(self);
    }

    fn pause(&mut self) {
        AyPlayer::pause(self);
    }

    fn stop(&mut self) {
        let _ = AyPlayer::stop(self);
    }

    fn state(&self) -> PlaybackState {
        self.state
    }

    fn metadata(&self) -> &Self::Metadata {
        &self.metadata
    }

    fn generate_samples_into(&mut self, buffer: &mut [f32]) {
        AyPlayer::generate_samples_into(self, buffer);
    }

    fn sample_rate(&self) -> u32 {
        SAMPLE_RATE
    }
}
