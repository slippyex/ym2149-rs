//! YM2149 PSG Chip Emulation
//!
//! Integer-accurate YM2149 core.
//!
//! This module mirrors the integer data-path of the original hardware so that
//! buzzer, noise and envelope behaviour matches the physical chip sample-by-sample.

use super::constants::VOLUME_TABLE;
use super::registers::RegisterBank;
use crate::backend::Ym2149Backend;
use std::env;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::sync::OnceLock;

const DC_BUFFER_LEN: usize = 512;

/// DC offset adjuster (moving average) operating on 16-bit PCM samples.
#[derive(Debug, Clone)]
struct DcAdjuster {
    buffer: [i32; DC_BUFFER_LEN],
    pos: usize,
    sum: i64,
}

impl DcAdjuster {
    fn new() -> Self {
        Self {
            buffer: [0; DC_BUFFER_LEN],
            pos: 0,
            sum: 0,
        }
    }

    fn reset(&mut self) {
        self.buffer.fill(0);
        self.pos = 0;
        self.sum = 0;
    }

    fn add_sample(&mut self, sample: i32) {
        self.sum -= self.buffer[self.pos] as i64;
        self.buffer[self.pos] = sample;
        self.sum += sample as i64;
        self.pos = (self.pos + 1) & (DC_BUFFER_LEN - 1);
    }

    fn get_level(&self) -> i32 {
        (self.sum / DC_BUFFER_LEN as i64) as i32
    }
}

#[derive(Debug)]
struct TraceState {
    writer: BufWriter<File>,
    sample_index: u64,
}

/// Mixer control overrides set by effects
#[derive(Debug, Clone, Copy)]
struct MixerOverrides {
    force_tone: [bool; 3],
    force_noise_mute: [bool; 3],
}

/// YM2149 Programmable Sound Generator core (integer-accurate).
#[derive(Debug)]
pub struct Ym2149 {
    registers: RegisterBank,
    master_clock: u32,
    sample_rate: u32,

    // Tone generators
    tone_pos: [u32; 3],
    tone_step: [u32; 3],
    tone_use_envelope: [bool; 3],
    tone_volume: [i32; 3],

    // Noise generator
    noise_step: u32,
    noise_pos: u32,
    rnd_rack: u32,
    current_noise: u32,

    // Envelope generator
    env_shape: usize,
    env_phase: usize,
    env_pos: u32,
    env_step: u32,
    env_volume: i32,

    // Mixer masks
    mixer_r7: u8,
    mixer_tone_mask: [u32; 3],
    mixer_noise_mask: [u32; 3],
    mixer_overrides: MixerOverrides,
    // User-controlled per-channel mutes (applied in addition to R7)
    user_mute: [bool; 3],

    // Effect overrides (digi-drum injection)
    drum_sample_overrides: [Option<i32>; 3],

    // Output conditioning
    dc_adjuster: DcAdjuster,
    low_pass: [i32; 2],
    color_filter_enabled: bool,

    // Cached envelope table
    env_data: &'static [[[u8; 32]; 2]; 16],

    // Output
    last_channel_output: [i32; 3],
    current_sample: f32,
    cycle_counter: u32,
    trace: Option<TraceState>,
    debug_sample_counter: u32, // DEBUG: for periodic output
}

impl Ym2149 {
    /// Create a new YM2149 instance with Atari ST defaults (2 MHz, 44.1 kHz).
    pub fn new() -> Self {
        Self::with_clocks(2_000_000, 44_100)
    }

    /// Create a YM2149 instance with custom clock and sample rate.
    pub fn with_clocks(master_clock: u32, sample_rate: u32) -> Self {
        let env_data = envelope_data();
        let mut chip = Self {
            registers: RegisterBank::new(),
            master_clock,
            sample_rate,
            tone_pos: [0; 3],
            tone_step: [0; 3],
            tone_use_envelope: [false; 3],
            tone_volume: [0; 3],
            noise_step: 0,
            noise_pos: 0,
            rnd_rack: 1,
            current_noise: 0xffff,
            env_shape: 0,
            env_phase: 0,
            env_pos: 0,
            env_step: 0,
            env_volume: 0,
            mixer_r7: 0xff,
            mixer_tone_mask: [0xffff; 3],
            mixer_noise_mask: [0xffff; 3],
            mixer_overrides: MixerOverrides {
                force_tone: [false; 3],
                force_noise_mute: [false; 3],
            },
            user_mute: [false; 3],
            drum_sample_overrides: [None; 3],
            dc_adjuster: DcAdjuster::new(),
            low_pass: [0; 2],
            color_filter_enabled: false,
            env_data,
            last_channel_output: [0; 3],
            current_sample: 0.0,
            cycle_counter: 0,
            trace: None,
            debug_sample_counter: 0,
        };
        chip.recompute_all_steps();
        chip.update_mixer_masks();
        if let Ok(path) = env::var("YM2149_TRACE")
            && let Ok(file) = File::create(path)
        {
            let mut writer = BufWriter::new(file);
            let _ = writeln!(
                writer,
                "sample,volA,volB,volC,volE,env_idx,env_phase,env_pos,noise,vol,in,out"
            );
            chip.trace = Some(TraceState {
                writer,
                sample_index: 0,
            });
        }
        chip
    }

    /// Reset to power-on state.
    pub fn reset(&mut self) {
        self.registers = RegisterBank::new();
        self.tone_pos = [0; 3];
        self.tone_step = [0; 3];
        self.tone_volume = [0; 3];
        self.tone_use_envelope = [false; 3];
        self.noise_step = 0;
        self.noise_pos = 0;
        self.rnd_rack = 1;
        self.current_noise = 0xffff;
        self.env_shape = 0;
        self.env_phase = 0;
        self.env_pos = 0;
        self.env_step = 0;
        self.env_volume = 0;
        self.mixer_r7 = 0xff;
        self.mixer_overrides.force_tone = [false; 3];
        self.mixer_overrides.force_noise_mute = [false; 3];
        self.user_mute = [false; 3];
        self.drum_sample_overrides = [None; 3];
        self.dc_adjuster.reset();
        self.low_pass = [0; 2];
        self.color_filter_enabled = true;
        self.last_channel_output = [0; 3];
        self.current_sample = 0.0;
        self.cycle_counter = 0;
        self.debug_sample_counter = 0;
        self.update_mixer_masks();
        self.recompute_all_steps();
    }

    /// Recompute tone/noise/envelope steps after clock configuration changes.
    fn recompute_all_steps(&mut self) {
        for ch in 0..3 {
            let lo = self.registers.read((ch * 2) as u8);
            let hi = self.registers.read((ch * 2 + 1) as u8);
            self.tone_step[ch] = tone_step_compute(lo, hi, self.master_clock, self.sample_rate);
            if self.tone_step[ch] == 0 {
                // Hardware-accurate special case: assume output always 1 if 0 period (for digi/sample paths)
                self.tone_pos[ch] = 1 << 31;
            }
        }

        let noise_value = self.registers.read(0x06);
        self.noise_step = noise_step_compute(noise_value, self.master_clock, self.sample_rate);
        if self.noise_step == 0 {
            self.current_noise = 0xffff;
            self.noise_pos = 0;
        }

        let env_lo = self.registers.read(0x0b);
        let env_hi = self.registers.read(0x0c);
        self.env_step = envelope_step_compute(env_lo, env_hi, self.master_clock, self.sample_rate);
    }

    /// Write to PSG register.
    pub fn write_register(&mut self, addr: u8, value: u8) {
        let idx = addr & 0x0F;
        self.registers.write(idx, value);

        match idx {
            0x00..=0x05 => {
                let channel = (idx / 2) as usize;
                let lo = self.registers.read((channel * 2) as u8);
                let hi = self.registers.read((channel * 2 + 1) as u8);
                self.tone_step[channel] =
                    tone_step_compute(lo, hi, self.master_clock, self.sample_rate);
                if self.tone_step[channel] == 0 {
                    // Hardware-accurate special case: assume output always 1 if 0 period
                    self.tone_pos[channel] = 1 << 31;
                }
            }
            0x06 => {
                self.noise_step = noise_step_compute(value, self.master_clock, self.sample_rate);
                if self.noise_step == 0 {
                    self.current_noise = 0xffff;
                    self.noise_pos = 0;
                }
            }
            0x07 => {
                self.mixer_r7 = value;
                self.update_mixer_masks();
            }
            0x08..=0x0A => {
                let channel = (idx - 0x08) as usize;
                self.tone_use_envelope[channel] = (value & 0x10) != 0;
                let vol_idx = (value & 0x0F) as usize;
                self.tone_volume[channel] = VOLUME_TABLE[vol_idx] as i32;
            }
            0x0B | 0x0C => {
                let env_lo = self.registers.read(0x0b);
                let env_hi = self.registers.read(0x0c);
                self.env_step =
                    envelope_step_compute(env_lo, env_hi, self.master_clock, self.sample_rate);
            }
            0x0D => {
                let new_shape = (value & 0x0F) as usize;
                // Writing R13 resets envelope position/phase on shape write (hardware behaviour)
                self.env_shape = new_shape;
                self.env_pos = 0;
                self.env_phase = 0;
            }
            _ => {}
        }
    }

    /// Read PSG register.
    pub fn read_register(&self, addr: u8) -> u8 {
        self.registers.read(addr & 0x0F)
    }

    /// Advance the PSG by one sample.
    #[allow(clippy::needless_range_loop)]
    pub fn clock(&mut self) {
        self.clock_noise();
        self.clock_envelope();

        let noise_mask = self.current_noise;

        let mut mixed = 0i32;
        let mut channel_outputs = [0i32; 3];
        for ch in 0..3 {
            if self.user_mute[ch] {
                channel_outputs[ch] = 0;
                self.last_channel_output[ch] = 0;
                continue;
            }
            if let Some(sample) = self.drum_sample_overrides[ch] {
                channel_outputs[ch] = sample;
                self.last_channel_output[ch] = sample;
                mixed += sample;
                continue;
            }

            let tone_state = ((self.tone_pos[ch] as i32) >> 31) as u32;
            let bt =
                (tone_state | self.mixer_tone_mask[ch]) & (noise_mask | self.mixer_noise_mask[ch]);

            let base_volume = if self.tone_use_envelope[ch] {
                self.env_volume
            } else {
                self.tone_volume[ch]
            };

            let channel_output = base_volume & bt as i32;
            channel_outputs[ch] = channel_output;
            self.last_channel_output[ch] = channel_output;
            mixed += channel_output;
        }

        let env_pos_value = self.env_pos;
        let env_index = env_pos_value >> (32 - 5);
        let env_phase_value = self.env_phase as u32;

        for ch in 0..3 {
            self.tone_pos[ch] = self.tone_pos[ch].wrapping_add(self.tone_step[ch]);
        }
        self.noise_pos = self.noise_pos.wrapping_add(self.noise_step);
        self.env_pos = self.env_pos.wrapping_add(self.env_step);
        if self.env_phase == 0 && self.env_step != 0 && self.env_pos < self.env_step {
            self.env_phase = 1;
        }

        self.dc_adjuster.add_sample(mixed);
        let dc_level = self.dc_adjuster.get_level();
        let in_value = mixed - dc_level;
        let mut out_value = in_value;

        if self.color_filter_enabled {
            let filtered = (self.low_pass[0] >> 2) + (self.low_pass[1] >> 1) + (in_value >> 2);
            self.low_pass[0] = self.low_pass[1];
            self.low_pass[1] = in_value;
            out_value = filtered;
        } else {
            self.low_pass[0] = in_value;
            self.low_pass[1] = in_value;
        }

        let clamped = out_value.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        self.current_sample = (clamped as f32) * (1.0 / i16::MAX as f32);

        if let Some(trace) = self.trace.as_mut() {
            let _ = writeln!(
                trace.writer,
                "{},{},{},{},{},{},{},{},{},{},{},{}",
                trace.sample_index,
                channel_outputs[0],
                channel_outputs[1],
                channel_outputs[2],
                self.env_volume,
                env_index,
                env_phase_value,
                env_pos_value,
                noise_mask & 0xffff,
                mixed,
                in_value,
                clamped as i32,
            );
            trace.sample_index += 1;
        }
        self.cycle_counter = self.cycle_counter.wrapping_add(1);
    }

    /// Current audio sample (-1.0..1.0).
    pub fn get_sample(&self) -> f32 {
        self.current_sample
    }

    /// Trigger envelope restart (used by sync buzzer effects).
    pub fn trigger_envelope(&mut self) {
        self.env_pos = 0;
        self.env_phase = 0;
    }

    /// Set envelope shape without resetting position/phase (used by Sync Buzzer start)
    pub fn set_envelope_shape_no_reset(&mut self, shape: u8) {
        self.env_shape = (shape & 0x0F) as usize;
    }

    /// Enable/disable colour (low-pass) filtering.
    pub fn set_color_filter(&mut self, enabled: bool) {
        self.color_filter_enabled = enabled;
    }

    /// Current channel outputs (for visualisation).
    pub fn get_channel_outputs(&self) -> (f32, f32, f32) {
        let norm = 1.0 / (i16::MAX as f32);
        (
            self.last_channel_output[0] as f32 * norm,
            self.last_channel_output[1] as f32 * norm,
            self.last_channel_output[2] as f32 * norm,
        )
    }

    /// Total number of generated samples since reset
    pub fn get_cycle_count(&self) -> u32 {
        self.cycle_counter
    }

    /// Load all 16 YM2149 registers from a frame
    pub fn load_registers(&mut self, regs: &[u8; 16]) {
        for (idx, &value) in regs.iter().enumerate() {
            self.write_register(idx as u8, value);
        }
    }

    /// Return a copy of all 16 YM2149 registers
    pub fn dump_registers(&self) -> [u8; 16] {
        *self.registers.as_slice()
    }

    /// Set mixer overrides for effects (used by EffectsManager in ym2149-ym-replayer)
    pub fn set_mixer_overrides(&mut self, force_tone: [bool; 3], force_noise_mute: [bool; 3]) {
        self.mixer_overrides.force_tone = force_tone;
        self.mixer_overrides.force_noise_mute = force_noise_mute;
        self.update_mixer_masks();
    }

    /// Set drum sample override for a voice (used by EffectsManager in ym2149-ym-replayer)
    pub fn set_drum_sample_override(&mut self, voice: usize, sample: Option<i32>) {
        if voice >= 3 {
            return;
        }
        self.drum_sample_overrides[voice] = sample;
    }

    #[allow(dead_code)] // clear_effect_overrides was removed as unused; effect paths explicitly set overrides.
    fn clock_noise(&mut self) {
        if self.noise_step != 0 && (self.noise_pos & 0xFFFF_0000) != 0 {
            self.current_noise ^= self.rnd_compute();
            self.noise_pos &= 0xFFFF;
        }
    }

    fn clock_envelope(&mut self) {
        let phase_data = &self.env_data[self.env_shape][self.env_phase];
        let idx = (self.env_pos >> (32 - 5)) as usize & 0x1F;
        let level = phase_data[idx] as usize;
        self.env_volume = VOLUME_TABLE[level] as i32;
    }

    fn update_mixer_masks(&mut self) {
        for ch in 0..3 {
            let tone_disabled = (self.mixer_r7 >> ch) & 0x01 != 0;
            let noise_disabled = (self.mixer_r7 >> (ch + 3)) & 0x01 != 0;

            // Apply user mute after considering overrides – mute forces full mask
            let tone_mask = tone_disabled && !self.mixer_overrides.force_tone[ch];
            self.mixer_tone_mask[ch] = if self.user_mute[ch] || tone_mask {
                0xffff
            } else {
                0
            };

            self.mixer_noise_mask[ch] = if self.user_mute[ch]
                || noise_disabled
                || self.mixer_overrides.force_noise_mute[ch]
            {
                0xffff
            } else {
                0
            };
        }
    }

    /// Mute or unmute a channel (0=A,1=B,2=C)
    pub fn set_channel_mute(&mut self, channel: usize, mute: bool) {
        if channel < 3 {
            self.user_mute[channel] = mute;
            self.update_mixer_masks();
        }
    }

    /// Get current mute state of a channel
    pub fn is_channel_muted(&self, channel: usize) -> bool {
        channel < 3 && self.user_mute[channel]
    }

    fn rnd_compute(&mut self) -> u32 {
        let r_bit = (self.rnd_rack & 1) ^ ((self.rnd_rack >> 2) & 1);
        self.rnd_rack = (self.rnd_rack >> 1) | (r_bit << 16);
        if r_bit != 0 { 0 } else { 0xffff }
    }
}

impl Default for Ym2149 {
    fn default() -> Self {
        Self::new()
    }
}

fn tone_step_compute(period_lo: u8, period_hi: u8, master_clock: u32, sample_rate: u32) -> u32 {
    let per = ((period_hi as u16 & 0x0F) << 8) | (period_lo as u16);
    if per <= 5 {
        return 0;
    }
    let per_u32 = per as u32;
    let numerator = (master_clock as u128) << (15 + 16 - 3);
    let denominator = (per_u32 as u128) * (sample_rate as u128);
    if denominator == 0 {
        0
    } else {
        (numerator / denominator) as u32
    }
}

fn noise_step_compute(register_value: u8, master_clock: u32, sample_rate: u32) -> u32 {
    let per = (register_value & 0x1F) as u32;
    if per < 3 {
        return 0;
    }
    let numerator = (master_clock as u128) << (16 - 1 - 3);
    let denominator = (per as u128) * (sample_rate as u128);
    if denominator == 0 {
        0
    } else {
        (numerator / denominator) as u32
    }
}

fn envelope_step_compute(lo: u8, hi: u8, master_clock: u32, sample_rate: u32) -> u32 {
    let per = (((hi as u16) << 8) | (lo as u16)) as u32;
    if per < 3 {
        return 0;
    }
    let numerator = (master_clock as u128) << (16 + 16 - 9);
    let denominator = (per as u128) * (sample_rate as u128);
    if denominator == 0 {
        0
    } else {
        (numerator / denominator) as u32
    }
}

static ENV_DATA: OnceLock<[[[u8; 32]; 2]; 16]> = OnceLock::new();

fn envelope_data() -> &'static [[[u8; 32]; 2]; 16] {
    ENV_DATA.get_or_init(build_envelope_data)
}

const ENV_WAVES: [[i32; 8]; 16] = [
    // 0x00-0x03: Attack-Decay (all the same)
    [1, 0, 0, 0, 0, 0, 0, 0],
    [1, 0, 0, 0, 0, 0, 0, 0],
    [1, 0, 0, 0, 0, 0, 0, 0],
    [1, 0, 0, 0, 0, 0, 0, 0],
    // 0x04-0x07: Attack-Sustain-Release variant (all the same)
    [0, 1, 0, 0, 0, 0, 0, 0],
    [0, 1, 0, 0, 0, 0, 0, 0],
    [0, 1, 0, 0, 0, 0, 0, 0],
    [0, 1, 0, 0, 0, 0, 0, 0],
    // 0x08: Sawtooth-Down repeating (BUZZER!)
    [1, 0, 1, 0, 1, 0, 1, 0],
    // 0x09: Attack-Sawtooth-Down
    [1, 0, 0, 0, 0, 0, 0, 0],
    // 0x0A: Sustain-Sawtooth-Down
    [1, 0, 0, 1, 1, 0, 0, 1],
    // 0x0B: Attack-Sustain-Sawtooth
    [1, 0, 1, 1, 1, 1, 1, 1],
    // 0x0C: Sawtooth-Up repeating (BUZZER!)
    [0, 1, 0, 1, 0, 1, 0, 1],
    // 0x0D: Attack-Hold
    [0, 1, 1, 1, 1, 1, 1, 1],
    // 0x0E: Sawtooth-Down once then silence
    [0, 1, 1, 0, 0, 1, 1, 0],
    // 0x0F: Attack-Hold (same as 0x0D)
    [0, 1, 0, 0, 0, 0, 0, 0],
];

fn build_envelope_data() -> [[[u8; 32]; 2]; 16] {
    let mut data = [[[0u8; 32]; 2]; 16];
    for (env_idx, wave) in ENV_WAVES.iter().enumerate() {
        for phase in 0..4 {
            let mut a = wave[phase * 2] * 15;
            let delta = wave[phase * 2 + 1] - wave[phase * 2];
            for step in 0..16 {
                let mut val = a;
                val = val.clamp(0, 15);
                let target_phase = phase / 2;
                let offset = (phase % 2) * 16 + step;
                data[env_idx][target_phase][offset] = val as u8;
                a += delta;
            }
        }
    }
    data
}

// Implement the Ym2149Backend trait for the hardware-accurate chip
impl Ym2149Backend for Ym2149 {
    fn new() -> Self {
        Ym2149::new()
    }

    fn with_clocks(master_clock: u32, sample_rate: u32) -> Self {
        Ym2149::with_clocks(master_clock, sample_rate)
    }

    fn reset(&mut self) {
        self.reset();
    }

    fn write_register(&mut self, addr: u8, value: u8) {
        self.write_register(addr, value);
    }

    fn read_register(&self, addr: u8) -> u8 {
        self.read_register(addr)
    }

    fn load_registers(&mut self, regs: &[u8; 16]) {
        self.load_registers(regs);
    }

    fn dump_registers(&self) -> [u8; 16] {
        self.dump_registers()
    }

    fn clock(&mut self) {
        self.clock();
    }

    fn get_sample(&self) -> f32 {
        self.get_sample()
    }

    fn get_channel_outputs(&self) -> (f32, f32, f32) {
        self.get_channel_outputs()
    }

    fn set_channel_mute(&mut self, channel: usize, mute: bool) {
        self.set_channel_mute(channel, mute);
    }

    fn is_channel_muted(&self, channel: usize) -> bool {
        self.is_channel_muted(channel)
    }

    fn set_color_filter(&mut self, enabled: bool) {
        self.set_color_filter(enabled);
    }

    fn trigger_envelope(&mut self) {
        Ym2149::trigger_envelope(self);
    }

    fn set_drum_sample_override(&mut self, channel: usize, sample: Option<f32>) {
        // Digi-drum path injects raw PCM-like values; keep scale and clamp to i32 range
        let sample_i32 = sample.map(|s| s as i32);
        Ym2149::set_drum_sample_override(self, channel, sample_i32);
    }

    fn set_mixer_overrides(&mut self, force_tone: [bool; 3], force_noise_mute: [bool; 3]) {
        Ym2149::set_mixer_overrides(self, force_tone, force_noise_mute);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tone_step_zero_period() {
        let step = tone_step_compute(0, 0, 2_000_000, 44_100);
        assert_eq!(step, 0);
    }

    #[test]
    fn test_volume_table_matches_reference() {
        assert_eq!(VOLUME_TABLE[0], 20);
        assert_eq!(VOLUME_TABLE[15], 10922);
    }

    #[test]
    fn test_envelope_generation() {
        let data = build_envelope_data();
        assert_eq!(data[0][0][0], 15);
        assert_eq!(data[0][0][31], 0);
    }

    #[test]
    fn test_clock_produces_output() {
        let mut chip = Ym2149::new();
        chip.write_register(0, 0x1C);
        chip.write_register(1, 0x01);
        chip.write_register(8, 0x0F);
        for _ in 0..10 {
            chip.clock();
        }
        let sample = chip.get_sample();
        assert!(sample.abs() > 0.0);
    }
}
