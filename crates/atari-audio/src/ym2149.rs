//! YM2149 PSG emulation - 1:1 port from AtariAudio
//!
//! Tiny & cycle accurate ym2149 emulation.
//! Operates at original YM freq divided by 8 (so 250Khz, as nothing runs faster in the chip)
//!
//! Original C++ by Arnaud Carré aka Leonard/Oxygene (@leonard_coder)

use crate::tables::{ENV_DATA, MASKS, REG_MASK, SHAPE_TO_ENV, YM2149_LOG_LEVELS};

const DC_ADJUST_HISTORY_BIT: usize = 11; // 2048 values (~20ms at 44Khz)
const DC_ADJUST_HISTORY_SIZE: usize = 1 << DC_ADJUST_HISTORY_BIT;

/// Simple PRNG for unpredictable power-on state
fn std_lib_rand(seed: &mut u32) -> u16 {
    *seed = seed.wrapping_mul(214013).wrapping_add(2531011);
    ((*seed >> 16) & 0x7fff) as u16
}

/// YM2149 PSG emulator - 1:1 port from AtariAudio's ym2149c
pub struct Ym2149c {
    selected_reg: usize,
    current_env_offset: usize, // Offset into ENV_DATA
    ym_clock_one_eighth: u32,
    host_replay_rate: u32,
    tone_counter: [u32; 3],
    tone_period: [u32; 3],
    tone_edges: u32,

    env_counter: u32,
    env_pos: i32,
    env_period: u32,
    noise_counter: u32,
    noise_period: u32,
    tone_mask: u32,
    noise_mask: u32,
    noise_rnd_rack: u32,
    current_noise_mask: u32,
    dc_adjust_buffer: [u16; DC_ADJUST_HISTORY_SIZE],
    dc_adjust_pos: usize,
    dc_adjust_sum: u32,
    regs: [u8; 14],
    inner_cycle: u32,
    noise_half: u32,
    inside_timer_irq: bool,
    edge_need_reset: [bool; 3],
}

impl Ym2149c {
    pub fn new() -> Self {
        let mut chip = Self {
            selected_reg: 0,
            current_env_offset: 0,
            ym_clock_one_eighth: 250_000,
            host_replay_rate: 44100,
            tone_counter: [0; 3],
            tone_period: [0; 3],
            tone_edges: 0,
            env_counter: 0,
            env_pos: 0,
            env_period: 0,
            noise_counter: 0,
            noise_period: 0,
            tone_mask: 0,
            noise_mask: 0,
            noise_rnd_rack: 1,
            current_noise_mask: 0,
            dc_adjust_buffer: [0; DC_ADJUST_HISTORY_SIZE],
            dc_adjust_pos: 0,
            dc_adjust_sum: 0,
            regs: [0; 14],
            inner_cycle: 0,
            noise_half: 0,
            inside_timer_irq: false,
            edge_need_reset: [false; 3],
        };
        chip.reset(44100, 2_000_000);
        chip
    }

    pub fn reset(&mut self, host_replay_rate: u32, ym_clock: u32) {
        let mut seed = 1u32;

        for v in 0..3 {
            self.tone_counter[v] = 0;
            self.tone_period[v] = 0;
        }

        // YM internal edge state are un-predictable
        self.tone_edges =
            (std_lib_rand(&mut seed) as u32 & ((1 << 10) | (1 << 5) | (1 << 0))) * 0x1f;

        self.inside_timer_irq = false;
        self.host_replay_rate = host_replay_rate;
        self.ym_clock_one_eighth = ym_clock / 8;
        self.noise_rnd_rack = 1;
        self.noise_half = 0;

        // Initialize registers (R7=0x3F, others=0)
        for r in 0..14 {
            let val = if r == 7 { 0x3f } else { 0 };
            self.write_reg(r, val);
        }

        self.selected_reg = 0;
        self.inner_cycle = 0;
        self.env_pos = 0;
        self.dc_adjust_pos = 0;
        self.dc_adjust_sum = 0;

        for i in 0..DC_ADJUST_HISTORY_SIZE {
            self.dc_adjust_buffer[i] = 0;
        }
    }

    pub fn write_port(&mut self, port: u8, value: u8) {
        if (port & 2) != 0 {
            self.write_reg(self.selected_reg, value);
        } else {
            self.selected_reg = (value as usize) & 0x0f;
        }
    }

    fn write_reg(&mut self, reg: usize, value: u8) {
        if reg < 14 {
            self.regs[reg] = value & REG_MASK[reg];

            match reg {
                0..=5 => {
                    let voice = reg >> 1;
                    self.tone_period[voice] =
                        ((self.regs[voice * 2 + 1] as u32) << 8) | self.regs[voice * 2] as u32;

                    if self.tone_period[voice] <= 1 && self.inside_timer_irq {
                        self.edge_need_reset[voice] = true;
                    }
                }
                6 => {
                    self.noise_period = self.regs[6] as u32;
                }
                7 => {
                    self.tone_mask = MASKS[(value & 0x7) as usize];
                    self.noise_mask = MASKS[((value >> 3) & 0x7) as usize];
                }
                11 | 12 => {
                    self.env_period = ((self.regs[12] as u32) << 8) | self.regs[11] as u32;
                }
                13 => {
                    let shape = (self.regs[13] & 0x0f) as usize;
                    self.current_env_offset = SHAPE_TO_ENV[shape] as usize * 32 * 4;
                    self.env_pos = -64;
                    self.env_counter = 0;
                }
                _ => {}
            }
        }
    }

    pub fn read_port(&self, port: u8) -> u8 {
        if (port & 2) == 0 {
            self.regs[self.selected_reg]
        } else {
            0xff
        }
    }

    fn dc_adjust(&mut self, v: u16) -> i16 {
        self.dc_adjust_sum -= self.dc_adjust_buffer[self.dc_adjust_pos] as u32;
        self.dc_adjust_sum += v as u32;
        self.dc_adjust_buffer[self.dc_adjust_pos] = v;
        self.dc_adjust_pos += 1;
        self.dc_adjust_pos &= DC_ADJUST_HISTORY_SIZE - 1;

        let ov = (v as i32) - ((self.dc_adjust_sum >> DC_ADJUST_HISTORY_BIT) as i32);
        // max amplitude is 15bits (not 16) so dc adjuster should never overshoot
        ov as i16
    }

    /// Tick internal YM2149 state machine at 250Khz (2Mhz/8)
    fn tick(&mut self) -> u16 {
        // Three voices at same time
        let vmask = (self.tone_edges | self.tone_mask) & (self.current_noise_mask | self.noise_mask);

        // Update internal state
        for v in 0..3 {
            self.tone_counter[v] += 1;
            if self.tone_counter[v] >= self.tone_period[v] {
                self.tone_edges ^= 0x1f << (v * 5);
                self.tone_counter[v] = 0;
            }
        }

        self.env_counter += 1;
        if self.env_counter >= self.env_period {
            self.env_pos += 1;
            if self.env_pos > 0 {
                self.env_pos &= 63;
            }
            self.env_counter = 0;
        }

        // Noise state machine is running half speed
        self.noise_half ^= 1;
        if self.noise_half != 0 {
            self.noise_counter += 1;
            if self.noise_counter >= self.noise_period {
                self.current_noise_mask =
                    if ((self.noise_rnd_rack ^ (self.noise_rnd_rack >> 2)) & 1) != 0 {
                        !0
                    } else {
                        0
                    };
                self.noise_rnd_rack =
                    (self.noise_rnd_rack >> 1) | ((self.current_noise_mask & 1) << 16);
                self.noise_counter = 0;
            }
        }

        vmask as u16
    }

    /// Called at host replay rate (like 48Khz)
    /// Internally updates YM chip state machine at 250Khz and averages output for each host sample
    pub fn compute_next_sample(&mut self) -> i16 {
        let mut high_mask: u16 = 0;

        loop {
            high_mask |= self.tick();
            self.inner_cycle += self.host_replay_rate;
            if self.inner_cycle >= self.ym_clock_one_eighth {
                break;
            }
        }
        self.inner_cycle -= self.ym_clock_one_eighth;

        // Get envelope level from table
        let env_level = ENV_DATA[self.current_env_offset + (self.env_pos + 64) as usize] as u32;

        // Build channel levels exactly like C++ reference
        let mut levels: u32 = 0;
        levels |= if (self.regs[8] & 0x10) != 0 {
            env_level
        } else {
            (self.regs[8] as u32) << 1
        };
        levels |= (if (self.regs[9] & 0x10) != 0 {
            env_level
        } else {
            (self.regs[9] as u32) << 1
        }) << 5;
        levels |= (if (self.regs[10] & 0x10) != 0 {
            env_level
        } else {
            (self.regs[10] as u32) << 1
        }) << 10;

        levels &= high_mask as u32;
        debug_assert!(levels < 0x8000);

        let half_shift_a = if self.tone_period[0] > 1 { 0 } else { 1 };
        let half_shift_b = if self.tone_period[1] > 1 { 0 } else { 1 };
        let half_shift_c = if self.tone_period[2] > 1 { 0 } else { 1 };

        let index_a = (levels >> 0) & 31;
        let index_b = (levels >> 5) & 31;
        let index_c = (levels >> 10) & 31;

        let level_a = YM2149_LOG_LEVELS[index_a as usize] >> half_shift_a;
        let level_b = YM2149_LOG_LEVELS[index_b as usize] >> half_shift_b;
        let level_c = YM2149_LOG_LEVELS[index_c as usize] >> half_shift_c;

        self.dc_adjust((level_a + level_b + level_c) as u16)
    }

    pub fn inside_timer_irq(&mut self, inside: bool) {
        if !inside {
            // When exiting timer IRQ code, do any pending edge reset ("square-sync" modern fx)
            for v in 0..3 {
                if self.edge_need_reset[v] {
                    self.tone_edges ^= 0x1f << (v * 5);
                    self.tone_counter[v] = 0;
                    self.edge_need_reset[v] = false;
                }
            }
        }
        self.inside_timer_irq = inside;
    }

    /// Read a YM register value (0-13)
    pub fn read_register(&self, reg: u8) -> u8 {
        if (reg as usize) < self.regs.len() {
            self.regs[reg as usize]
        } else {
            0
        }
    }
}

impl Default for Ym2149c {
    fn default() -> Self {
        Self::new()
    }
}
