//! YM2149 PSG core (clk/8 stepping)
//!
//! Design highlights:
//! - clk/8 internal stepping (~250 kHz @ 2 MHz) with host-rate averaging
//! - Hardware envelope/volume tables (10 shapes, 32-step volume)
//! - Half-rate noise LFSR, randomized power-on tone edges
//! - DC adjust ring buffer to remove offset drift
//! - DigiDrum/SID/buzzer hooks via mixer overrides and drum injection
//! - Empiric DAC table for authentic Atari ST audio mixing
//! - Matches hardware behaviour as closely as practical for buzzer and digidrums

use super::empiric_dac::empiric_dac_lookup;
use crate::backend::Ym2149Backend;

const DEFAULT_MASTER_CLOCK: u32 = 2_000_000;
const DEFAULT_SAMPLE_RATE: u32 = 44_100;
const YM_DIVIDER: u32 = 8;
const DC_HISTORY_BITS: usize = 11; // 2048 samples (~20 ms @ 44.1 kHz)
const DC_HISTORY_LEN: usize = 1 << DC_HISTORY_BITS;

/// Small DC offset remover using a sliding window
#[derive(Debug, Clone)]
struct DcAdjuster {
    buf: [i32; DC_HISTORY_LEN],
    pos: usize,
    sum: i64,
}

impl DcAdjuster {
    fn new() -> Self {
        Self {
            buf: [0; DC_HISTORY_LEN],
            pos: 0,
            sum: 0,
        }
    }

    fn push(&mut self, sample: i32) {
        self.sum -= self.buf[self.pos] as i64;
        self.buf[self.pos] = sample;
        self.sum += sample as i64;
        self.pos = (self.pos + 1) & (DC_HISTORY_LEN - 1);
    }

    fn level(&self) -> i32 {
        (self.sum / DC_HISTORY_LEN as i64) as i32
    }
}

/// Hardware envelope tables (10 shapes × 128 entries).
fn envelope_data() -> &'static [[u8; 128]; 10] {
    static ENV_TABLE: [[u8; 128]; 10] = [
        [
            31, 30, 29, 28, 27, 26, 25, 24, 23, 22, 21, 20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10,
            9, 8, 7, 6, 5, 4, 3, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ],
        [
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24, 25, 26, 27, 28, 29, 30, 31, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ],
        [
            31, 30, 29, 28, 27, 26, 25, 24, 23, 22, 21, 20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10,
            9, 8, 7, 6, 5, 4, 3, 2, 1, 0, 31, 30, 29, 28, 27, 26, 25, 24, 23, 22, 21, 20, 19, 18,
            17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0, 31, 30, 29, 28, 27, 26,
            25, 24, 23, 22, 21, 20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2,
            1, 0, 31, 30, 29, 28, 27, 26, 25, 24, 23, 22, 21, 20, 19, 18, 17, 16, 15, 14, 13, 12,
            11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0,
        ],
        [
            31, 30, 29, 28, 27, 26, 25, 24, 23, 22, 21, 20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10,
            9, 8, 7, 6, 5, 4, 3, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ],
        [
            31, 30, 29, 28, 27, 26, 25, 24, 23, 22, 21, 20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10,
            9, 8, 7, 6, 5, 4, 3, 2, 1, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16,
            17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 31, 30, 29, 28, 27, 26, 25,
            24, 23, 22, 21, 20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1,
            0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
            23, 24, 25, 26, 27, 28, 29, 30, 31,
        ],
        [
            31, 30, 29, 28, 27, 26, 25, 24, 23, 22, 21, 20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10,
            9, 8, 7, 6, 5, 4, 3, 2, 1, 0, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31,
            31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31,
            31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31,
            31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31,
            31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31,
        ],
        [
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24, 25, 26, 27, 28, 29, 30, 31, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
            16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 0, 1, 2, 3, 4, 5, 6, 7,
            8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29,
            30, 31, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
            22, 23, 24, 25, 26, 27, 28, 29, 30, 31,
        ],
        [
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24, 25, 26, 27, 28, 29, 30, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31,
            31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31,
            31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31,
            31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31,
            31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31,
        ],
        [
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24, 25, 26, 27, 28, 29, 30, 31, 31, 30, 29, 28, 27, 26, 25, 24, 23, 22, 21, 20, 19, 18,
            17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0, 0, 1, 2, 3, 4, 5, 6, 7,
            8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29,
            30, 31, 31, 30, 29, 28, 27, 26, 25, 24, 23, 22, 21, 20, 19, 18, 17, 16, 15, 14, 13, 12,
            11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0,
        ],
        [
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24, 25, 26, 27, 28, 29, 30, 31, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ],
    ];
    &ENV_TABLE
}

/// Hardware 32-step volume table (5-bit).
/// Values from C++ reference: s_ym2149LogLevels[i] = raw[i] / 3
/// where raw values are computed using 1.f / powf(sqrtf(2.f), level * 0.5f)
/// Pre-divided by 3 for headroom (3 channels can sum to ~32767).
fn volume_table_32() -> &'static [i32; 32] {
    static TABLE: [i32; 32] = [
        50, 60, 71, 85, 101, 120, 143, 170, 202, 241, 287, 341, 405, 482, 574, 682, 811, 965, 1148,
        1365, 1623, 1930, 2296, 2730, 3247, 3861, 4592, 5461, 6494, 7723, 9184, 10922,
    ];
    &TABLE
}

/// YM2149 core implementation (clk/8 stepping).
#[derive(Debug, Clone)]
pub struct Ym2149 {
    regs: [u8; 16],
    tone_counter: [u32; 3],
    tone_period: [u32; 3],
    tone_edges: u32,
    tone_mask: u32,
    noise_mask: u32,

    noise_counter: u32,
    noise_period: u32,
    noise_rng: u32,
    noise_half: bool,
    current_noise_mask: u32,

    env_counter: u32,
    env_period: u32,
    env_pos: i32,
    env_shape: usize,
    env_data: &'static [[u8; 128]; 10],

    host_rate: u32,
    ym_clock_one_eighth: u32,
    inner_cycle: u32,

    dc_adjust: DcAdjuster,
    user_mute: [bool; 3],
    mixer_overrides: MixerOverrides,
    drum_overrides: [Option<i32>; 3],

    last_channels: [f32; 3],
    last_sample: f32,

    // Square-sync buzzer support (SID voice effects)
    inside_timer_irq: bool,
    edge_need_reset: [bool; 3],
}

#[derive(Debug, Clone, Copy, Default)]
struct MixerOverrides {
    force_tone: [bool; 3],
    force_noise_mute: [bool; 3],
}

impl Ym2149 {
    fn rng_step(seed: &mut u32) -> u16 {
        *seed = seed.wrapping_mul(214013).wrapping_add(2531011);
        ((*seed >> 16) & 0x7fff) as u16
    }

    fn new_inner(master_clock: u32, sample_rate: u32) -> Self {
        let env_data = envelope_data();
        let mut seed = 1u32;
        let tone_edges =
            (Self::rng_step(&mut seed) as u32 & ((1 << 10) | (1 << 5) | (1 << 0))) * 0x1f;
        let ym_clock_one_eighth = master_clock / YM_DIVIDER;

        let mut chip = Self {
            regs: [0; 16],
            tone_counter: [0; 3],
            tone_period: [0; 3],
            tone_edges,
            tone_mask: 0,
            noise_mask: 0,
            noise_counter: 0,
            noise_period: 0,
            noise_rng: 1,
            noise_half: false,
            current_noise_mask: 0,
            env_counter: 0,
            env_period: 0,
            env_pos: -64,
            env_shape: 0,
            env_data,
            host_rate: sample_rate,
            ym_clock_one_eighth,
            inner_cycle: 0,
            dc_adjust: DcAdjuster::new(),
            user_mute: [false; 3],
            mixer_overrides: MixerOverrides::default(),
            drum_overrides: [None, None, None],
            last_channels: [0.0; 3],
            last_sample: 0.0,
            inside_timer_irq: false,
            edge_need_reset: [false; 3],
        };
        // YM power-on state: R7=0x3F, others 0
        for r in 0..14 {
            let val = if r == 7 { 0x3f } else { 0 };
            chip.write_register(r as u8, val);
        }
        chip
    }

    fn dc_adjust_sample(&mut self, sample: i32) -> i32 {
        self.dc_adjust.push(sample);
        sample - self.dc_adjust.level()
    }

    fn envelope_level(&self) -> u8 {
        let pos = ((self.env_pos + 64) & 0x7f) as usize;
        self.env_data[self.env_shape][pos]
    }

    fn update_masks(&mut self) {
        const MASKS: [u32; 8] = [
            0x0000, 0x001f, 0x03e0, 0x03ff, 0x7c00, 0x7c1f, 0x7fe0, 0x7fff,
        ];
        let r7 = self.regs[7];
        let tone_idx = r7 & 0x7;
        let noise_idx = (r7 >> 3) & 0x7;
        self.tone_mask = MASKS[tone_idx as usize];
        self.noise_mask = MASKS[noise_idx as usize];
    }

    fn tick(&mut self) -> u32 {
        let vmask =
            (self.tone_edges | self.tone_mask) & (self.current_noise_mask | self.noise_mask);

        for v in 0..3 {
            self.tone_counter[v] = self.tone_counter[v].wrapping_add(1);
            if self.tone_counter[v] >= self.tone_period[v] {
                self.tone_edges ^= 0x1f << (v * 5);
                self.tone_counter[v] = 0;
            }
        }

        self.env_counter = self.env_counter.wrapping_add(1);
        if self.env_counter >= self.env_period {
            self.env_pos = self.env_pos.wrapping_add(1);
            if self.env_pos > 0 {
                self.env_pos &= 0x3f;
            }
            self.env_counter = 0;
        }

        self.noise_half = !self.noise_half;
        if self.noise_half {
            self.noise_counter = self.noise_counter.wrapping_add(1);
            if self.noise_counter >= self.noise_period {
                let bit = (self.noise_rng ^ (self.noise_rng >> 2)) & 1;
                self.current_noise_mask = if bit != 0 { u32::MAX } else { 0 };
                self.noise_rng = (self.noise_rng >> 1) | ((self.current_noise_mask & 1) << 16);
                self.noise_counter = 0;
            }
        }

        vmask
    }

    fn compute_sample(&mut self) -> (f32, [f32; 3]) {
        let mut high_mask = 0u32;
        loop {
            high_mask |= self.tick();
            self.inner_cycle = self.inner_cycle.wrapping_add(self.host_rate);
            if self.inner_cycle >= self.ym_clock_one_eighth {
                break;
            }
        }
        self.inner_cycle = self.inner_cycle.wrapping_sub(self.ym_clock_one_eighth);

        let env_level = self.envelope_level() as u32;

        // Build channel levels exactly like C++ reference:
        // levels  = ((m_regs[8] & 0x10) ? envLevel : (m_regs[8]<<1)) << 0;
        // levels |= ((m_regs[9] & 0x10) ? envLevel : (m_regs[9]<<1)) << 5;
        // levels |= ((m_regs[10] & 0x10) ? envLevel : (m_regs[10]<<1)) << 10;
        let make_level = |reg: u8| -> u32 {
            if (reg & 0x10) != 0 {
                env_level
            } else {
                ((reg & 0x0f) << 1) as u32
            }
        };

        let mut levels: u32 = 0;
        levels |= make_level(self.regs[8]);
        levels |= make_level(self.regs[9]) << 5;
        levels |= make_level(self.regs[10]) << 10;

        // Apply user mute
        for (i, muted) in self.user_mute.iter().enumerate() {
            if *muted {
                levels &= !(0x1f << (i * 5));
            }
        }

        // Apply mixer overrides
        if self.mixer_overrides.force_tone.iter().any(|&f| f) {
            for (i, force) in self.mixer_overrides.force_tone.iter().enumerate() {
                if *force {
                    high_mask |= 0x1f << (i * 5);
                }
            }
        }
        if self.mixer_overrides.force_noise_mute.iter().any(|&m| m) {
            for (i, mute) in self.mixer_overrides.force_noise_mute.iter().enumerate() {
                if *mute {
                    high_mask &= !(0x1f << (i * 5));
                }
            }
        }

        // C++ reference: levels &= highMask;
        // This masks the volume levels with the tone/noise output state
        levels &= high_mask;

        // Extract indices for each channel (0-31)
        let idx_a = (levels & 0x1f) as usize;
        let idx_b = ((levels >> 5) & 0x1f) as usize;
        let idx_c = ((levels >> 10) & 0x1f) as usize;

        // Check for drum overrides - if any channel has a drum override,
        // we need to use linear mixing for that channel
        let has_drum_override =
            self.drum_overrides[0].is_some() || self.drum_overrides[1].is_some() || self.drum_overrides[2].is_some();

        let (mixed, per_channel) = if has_drum_override {
            // Fallback to linear mixing when drum samples are active
            let vol32 = volume_table_32();
            let half_shift = |period: u32| if period > 1 { 0 } else { 1 };

            let level_a = if let Some(override_sample) = self.drum_overrides[0] {
                override_sample
            } else {
                (vol32[idx_a] >> half_shift(self.tone_period[0])) as i32
            };
            let level_b = if let Some(override_sample) = self.drum_overrides[1] {
                override_sample
            } else {
                (vol32[idx_b] >> half_shift(self.tone_period[1])) as i32
            };
            let level_c = if let Some(override_sample) = self.drum_overrides[2] {
                override_sample
            } else {
                (vol32[idx_c] >> half_shift(self.tone_period[2])) as i32
            };

            let mixed = level_a + level_b + level_c;
            (
                mixed,
                [
                    level_a as f32 / 32767.0,
                    level_b as f32 / 32767.0,
                    level_c as f32 / 32767.0,
                ],
            )
        } else {
            // Use empiric DAC table for authentic Atari ST audio mixing
            // This models the non-linear behavior of the resistor network
            let mixed = empiric_dac_lookup(idx_a, idx_b, idx_c);

            // For per-channel outputs, use linear approximation
            let vol32 = volume_table_32();
            (
                mixed,
                [
                    vol32[idx_a] as f32 / 32767.0,
                    vol32[idx_b] as f32 / 32767.0,
                    vol32[idx_c] as f32 / 32767.0,
                ],
            )
        };

        let adjusted = self.dc_adjust_sample(mixed);
        let norm = adjusted as f32 / 32767.0;
        (norm, per_channel)
    }
}

impl Ym2149Backend for Ym2149 {
    fn new() -> Self {
        Self::with_clocks(DEFAULT_MASTER_CLOCK, DEFAULT_SAMPLE_RATE)
    }

    fn with_clocks(master_clock: u32, sample_rate: u32) -> Self {
        Self::new_inner(master_clock, sample_rate)
    }

    fn reset(&mut self) {
        *self = Self::with_clocks(self.ym_clock_one_eighth * YM_DIVIDER, self.host_rate);
    }

    fn write_register(&mut self, addr: u8, value: u8) {
        if (addr as usize) >= self.regs.len() {
            return;
        }
        const REG_MASKS: [u8; 14] = [
            0xff, 0x0f, 0xff, 0x0f, 0xff, 0x0f, 0x1f, 0x3f, 0x1f, 0x1f, 0x1f, 0xff, 0xff, 0x0f,
        ];
        let masked = if (addr as usize) < REG_MASKS.len() {
            value & REG_MASKS[addr as usize]
        } else {
            value
        };
        self.regs[addr as usize] = masked;

        match addr {
            0..=5 => {
                let voice = (addr / 2) as usize;
                let period = ((self.regs[voice * 2 + 1] as u32) << 8) | self.regs[voice * 2] as u32;
                self.tone_period[voice] = period;
                // Square-sync buzzer effect: when period <= 1 inside timer IRQ,
                // schedule an edge reset for when we exit the IRQ
                if period <= 1 && self.inside_timer_irq {
                    self.edge_need_reset[voice] = true;
                }
            }
            6 => {
                self.noise_period = self.regs[6] as u32;
            }
            7 => {
                self.update_masks();
            }
            11 | 12 => {
                // C++ reference: m_envPeriod = (m_regs[12] << 8) | m_regs[11];
                // Period 0 is valid and means envelope advances every YM tick (buzzer effects)
                let raw = ((self.regs[12] as u32) << 8) | self.regs[11] as u32;
                self.env_period = raw;
            }
            13 => {
                const SHAPE_MAP: [usize; 16] = [0, 0, 0, 0, 1, 1, 1, 1, 2, 3, 4, 5, 6, 7, 8, 9];
                let raw = (self.regs[13] & 0x0f) as usize;
                self.env_shape = SHAPE_MAP[raw];
                self.env_pos = -64;
                self.env_counter = 0;
            }
            _ => {}
        }
    }

    fn read_register(&self, addr: u8) -> u8 {
        self.regs.get(addr as usize).copied().unwrap_or(0)
    }

    fn load_registers(&mut self, regs: &[u8; 16]) {
        for (i, &v) in regs.iter().enumerate() {
            self.write_register(i as u8, v);
        }
    }

    fn dump_registers(&self) -> [u8; 16] {
        self.regs
    }

    fn clock(&mut self) {
        let (sample, channels) = self.compute_sample();
        self.last_sample = sample;
        self.last_channels = channels;
    }

    fn get_sample(&self) -> f32 {
        self.last_sample
    }

    fn get_channel_outputs(&self) -> (f32, f32, f32) {
        (
            self.last_channels[0],
            self.last_channels[1],
            self.last_channels[2],
        )
    }

    fn set_channel_mute(&mut self, channel: usize, mute: bool) {
        if channel < self.user_mute.len() {
            self.user_mute[channel] = mute;
        }
    }

    fn is_channel_muted(&self, channel: usize) -> bool {
        self.user_mute.get(channel).copied().unwrap_or(false)
    }

    fn set_color_filter(&mut self, _enabled: bool) {
        // No post filter in this backend.
    }

    fn trigger_envelope(&mut self) {
        self.env_pos = -64;
        self.env_counter = 0;
    }

    fn set_drum_sample_override(&mut self, channel: usize, sample: Option<f32>) {
        if channel < self.drum_overrides.len() {
            self.drum_overrides[channel] = sample.map(|s| s.round() as i32);
        }
    }

    fn set_mixer_overrides(&mut self, force_tone: [bool; 3], force_noise_mute: [bool; 3]) {
        self.mixer_overrides = MixerOverrides {
            force_tone,
            force_noise_mute,
        };
    }
}

// Inherent helpers mirroring the legacy Ym2149 API.
impl Ym2149 {
    /// Create a chip with Atari ST defaults (2 MHz, 44.1 kHz).
    pub fn new() -> Self {
        <Self as Ym2149Backend>::new()
    }
    /// Create a chip with custom master clock and sample rate.
    pub fn with_clocks(master_clock: u32, sample_rate: u32) -> Self {
        <Self as Ym2149Backend>::with_clocks(master_clock, sample_rate)
    }
    /// Reset chip state.
    pub fn reset(&mut self) {
        <Self as Ym2149Backend>::reset(self)
    }
    /// Write a PSG register (R0-R15).
    pub fn write_register(&mut self, addr: u8, value: u8) {
        <Self as Ym2149Backend>::write_register(self, addr, value)
    }
    /// Read a PSG register.
    pub fn read_register(&self, addr: u8) -> u8 {
        <Self as Ym2149Backend>::read_register(self, addr)
    }
    /// Load all 16 registers.
    pub fn load_registers(&mut self, regs: &[u8; 16]) {
        <Self as Ym2149Backend>::load_registers(self, regs)
    }
    /// Dump all 16 registers.
    pub fn dump_registers(&self) -> [u8; 16] {
        <Self as Ym2149Backend>::dump_registers(self)
    }
    /// Advance one host sample, internally stepping YM at clk/8.
    pub fn clock(&mut self) {
        <Self as Ym2149Backend>::clock(self)
    }
    /// Get last mixed sample (-1.0..1.0).
    pub fn get_sample(&self) -> f32 {
        <Self as Ym2149Backend>::get_sample(self)
    }
    /// Get per-channel outputs (A,B,C) normalized to [-1,1].
    pub fn get_channel_outputs(&self) -> (f32, f32, f32) {
        <Self as Ym2149Backend>::get_channel_outputs(self)
    }
    /// Mute/unmute a channel (0=A,1=B,2=C).
    pub fn set_channel_mute(&mut self, channel: usize, mute: bool) {
        <Self as Ym2149Backend>::set_channel_mute(self, channel, mute)
    }
    /// Check if a channel is muted.
    pub fn is_channel_muted(&self, channel: usize) -> bool {
        <Self as Ym2149Backend>::is_channel_muted(self, channel)
    }
    /// Enable/disable color filter (no-op in this backend).
    pub fn set_color_filter(&mut self, enabled: bool) {
        <Self as Ym2149Backend>::set_color_filter(self, enabled)
    }
    /// Restart envelope (used by buzzer effects).
    pub fn trigger_envelope(&mut self) {
        <Self as Ym2149Backend>::trigger_envelope(self)
    }
    /// Inject a drum sample (Digidrums).
    pub fn set_drum_sample_override(&mut self, channel: usize, sample: Option<f32>) {
        <Self as Ym2149Backend>::set_drum_sample_override(self, channel, sample)
    }
    /// Force mixer tone/noise bits (effects).
    pub fn set_mixer_overrides(&mut self, force_tone: [bool; 3], force_noise_mute: [bool; 3]) {
        <Self as Ym2149Backend>::set_mixer_overrides(self, force_tone, force_noise_mute)
    }
    /// Signal entry/exit of timer IRQ for square-sync buzzer effects.
    ///
    /// When exiting the timer IRQ (inside=false), any pending edge resets
    /// are applied. This is used by SID voice effects where the tone period
    /// is set to 0 or 1 inside the IRQ to create sample-accurate waveforms.
    pub fn set_inside_timer_irq(&mut self, inside: bool) {
        if !inside {
            // When exiting timer IRQ, apply any pending edge resets
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
}

impl Default for Ym2149 {
    fn default() -> Self {
        Self::new()
    }
}
