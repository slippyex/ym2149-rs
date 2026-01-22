//! Atari STE DMA Sound (DAC) emulation.
//!
//! The Atari STE introduced DMA-based digital audio playback, allowing
//! 8-bit mono or stereo samples to be played directly from memory without
//! CPU intervention.
//!
//! ## Features
//!
//! - Sample rates: 6.25 kHz, 12.5 kHz, 25 kHz, or 50 kHz
//! - Mono or stereo playback
//! - Loop mode for continuous playback
//! - Microwire interface for volume control
//!
//! ## Memory Map
//!
//! The STE sound hardware is mapped at 0xFF8900-0xFF893F:
//! - 0xFF8901: Sound control (bit 0 = enable)
//! - 0xFF8903-0xFF8907: Sample start address
//! - 0xFF8909-0xFF890D: Current sample pointer
//! - 0xFF890F-0xFF8913: Sample end address
//! - 0xFF8921: Sound mode (frequency, mono/stereo)
//! - 0xFF8922-0xFF8925: Microwire interface
//!
//! ## Special Support
//!
//! This implementation supports the "Tao MS3" driver technique which
//! outputs 4 interleaved voice samples at 50 kHz for software mixing.

use crate::mfp68901::Mfp68901;

const STE_DAC_FRQ: u32 = 50066;

/// DAC frequency divisors
const DAC_FREQ: [u32; 4] = [
    STE_DAC_FRQ / 8,
    STE_DAC_FRQ / 4,
    STE_DAC_FRQ / 2,
    STE_DAC_FRQ,
];

/// STE DMA Sound emulation
pub struct SteDac {
    host_replay_rate: u32,
    sample_ptr: u32,
    sample_end_ptr: u32,
    inner_clock: u32,
    microwire_mask: u16,
    microwire_data: u16,
    microwire_shift: i32,
    regs: [u8; 256],
    master_volume: i32,
    /// 50kHz to 25kHz averaging toggle
    flip_50_to_25: bool,
    /// Accumulator for 50kHz mode (left channel)
    acc_50_l: i32,
    /// Accumulator for 50kHz mode (right channel)
    acc_50_r: i32,
    /// Current DAC level (left channel)
    current_dac_level_l: i16,
    /// Current DAC level (right channel)
    current_dac_level_r: i16,
    /// Left channel mute flag
    mute_left: bool,
    /// Right channel mute flag
    mute_right: bool,
    /// Track if STE DAC has ever been used (registers written/playback started)
    was_used: bool,
    /// Last output sample (left) - for visualization
    last_output_l: i16,
    /// Last output sample (right) - for visualization
    last_output_r: i16,
}

impl SteDac {
    pub fn new(host_replay_rate: u32) -> Self {
        let mut dac = Self {
            host_replay_rate,
            sample_ptr: 0,
            sample_end_ptr: 0,
            inner_clock: 0,
            microwire_mask: 0,
            microwire_data: 0,
            microwire_shift: 0,
            regs: [0; 256],
            master_volume: 64,
            flip_50_to_25: false,
            acc_50_l: 0,
            acc_50_r: 0,
            current_dac_level_l: 0,
            current_dac_level_r: 0,
            mute_left: false,
            mute_right: false,
            was_used: false,
            last_output_l: 0,
            last_output_r: 0,
        };
        dac.reset(host_replay_rate);
        dac
    }

    pub fn reset(&mut self, host_replay_rate: u32) {
        for i in 0..256 {
            self.regs[i] = 0;
        }
        self.host_replay_rate = host_replay_rate;
        self.sample_ptr = 0;
        self.inner_clock = 0;
        self.microwire_mask = 0;
        self.microwire_shift = 0;
        self.microwire_data = 0;
        self.master_volume = 64;
        self.current_dac_level_l = 0;
        self.current_dac_level_r = 0;
        self.acc_50_l = 0;
        self.acc_50_r = 0;
        self.flip_50_to_25 = false;
        // Note: was_used and mute state preserved across reset for detection purposes
    }

    /// Check if STE DAC has been used (DMA playback activated).
    ///
    /// This is set when the 68000 code enables DMA playback.
    /// Used for runtime STE feature detection.
    pub fn was_used(&self) -> bool {
        self.was_used
    }

    /// Mute or unmute the left DAC channel.
    pub fn set_mute_left(&mut self, mute: bool) {
        self.mute_left = mute;
    }

    /// Mute or unmute the right DAC channel.
    pub fn set_mute_right(&mut self, mute: bool) {
        self.mute_right = mute;
    }

    /// Check if left channel is muted.
    pub fn is_left_muted(&self) -> bool {
        self.mute_left
    }

    /// Check if right channel is muted.
    pub fn is_right_muted(&self) -> bool {
        self.mute_right
    }

    /// Get current DAC levels for visualization (normalized 0.0 to 1.0).
    pub fn get_levels(&self) -> (f32, f32) {
        // Normalize from i16 range to 0.0-1.0
        // DAC levels are typically in range -8192 to 8192 (with master volume)
        let max_level = 8192.0;
        let left = (self.last_output_l.abs() as f32 / max_level).min(1.0);
        let right = (self.last_output_r.abs() as f32 / max_level).min(1.0);
        (left, right)
    }

    fn fetch_sample_ptr(&mut self) {
        self.sample_ptr = ((self.regs[3] as u32) << 16)
            | ((self.regs[5] as u32) << 8)
            | ((self.regs[7] & 0xfe) as u32);
        self.sample_end_ptr = ((self.regs[0x0f] as u32) << 16)
            | ((self.regs[0x11] as u32) << 8)
            | ((self.regs[0x13] & 0xfe) as u32);
    }

    pub fn write8(&mut self, ad: u8, data: u8) {
        let ad = ad as usize & 0xff;
        if (ad & 1) != 0 {
            let mut data = data;
            match ad {
                0x01 => {
                    if (data & 1) != 0 && ((data ^ self.regs[1]) & 1) != 0 {
                        // Replay just started
                        self.fetch_sample_ptr();
                        self.was_used = true;
                    }
                }
                0x07 | 0x0d => {
                    data &= 0xfe;
                }
                0x21 => {
                    if (data & 3) != (self.regs[0x21] & 3) {
                        self.acc_50_l = 0;
                        self.acc_50_r = 0;
                        self.flip_50_to_25 = false;
                    }
                }
                _ => {}
            }
            self.regs[ad] = data;
        }
    }

    pub fn write16(&mut self, ad: u8, data: u16) {
        let ad = ad as usize & 0xff;
        if (ad & 1) == 0 {
            match ad {
                0x22 => {
                    self.microwire_data = data;
                    self.microwire_proceed();
                    self.microwire_shift = 16;
                }
                0x24 => {
                    self.microwire_mask = data;
                }
                _ => {
                    self.write8((ad + 1) as u8, data as u8);
                }
            }
        }
    }

    pub fn read8(&self, ad: u8) -> u8 {
        let ad = ad as usize & 0xff;
        let mut data = 0xff;
        if (ad & 1) != 0 {
            data = self.regs[ad];
            match ad {
                0x09 => data = (self.sample_ptr >> 16) as u8,
                0x0b => data = (self.sample_ptr >> 8) as u8,
                0x0d => data = self.sample_ptr as u8,
                _ => {}
            }
        }
        data
    }

    pub fn read16(&mut self, ad: u8) -> u16 {
        let ad = ad as usize & 0xff;
        if (ad & 1) == 0 {
            match ad {
                0x22 => self.microwire_data,
                0x24 => self.microwire_tick(),
                _ => 0xff00 | self.read8((ad + 1) as u8) as u16,
            }
        } else {
            0xffff
        }
    }

    fn fetch_sample(ram: &[u8], atari_ad: u32) -> i8 {
        if (atari_ad as usize) < ram.len() {
            ram[atari_ad as usize] as i8
        } else {
            0
        }
    }

    /// Compute next DAC stereo sample
    ///
    /// Supports tricky Tao "MS3" driver. Seems to be a 3 or 4 voices synth, without need of mixing code!
    /// The 4 voices are just output in 4 consecutive bytes. Everything is playing at 50Khz, stereo
    /// On real hardware with analog filters & friends, it "sounds" like if you mixed 4 voices at 25Khz
    ///
    /// ComputeNextSample is called at host rate
    /// but the while loop is running at DAC speed. In 50khz mode, 2 samples are accumulated before
    /// output. So you get a mixed stream at 25Khz. None of original atari samples are missed, and
    /// Tao MS3 songs are playing ok!
    /// Please note it also works perfectly with Quartet STE code, that is mixing into a 2 bytes 50Khz buffer!! :)
    ///
    /// Returns (left, right) stereo samples.
    pub fn compute_sample_stereo(&mut self, atari_ram: &[u8], mfp: &mut Mfp68901) -> (i16, i16) {
        if (self.regs[1] & 1) != 0 {
            self.inner_clock += DAC_FREQ[(self.regs[0x21] & 3) as usize];
            let stereo = (self.regs[0x21] & 0x80) == 0;
            let b50k = (self.regs[0x21] & 3) == 3;

            while self.inner_clock >= self.host_replay_rate {
                if self.sample_ptr == self.sample_end_ptr {
                    mfp.set_ste_dac_external_event();
                    self.fetch_sample_ptr();
                    if (self.regs[0x01] & (1 << 1)) == 0 {
                        // If no loop mode, switch off replay
                        self.regs[0x01] &= 0xfe;
                        self.current_dac_level_l = 0;
                        self.current_dac_level_r = 0;
                        break;
                    }
                }

                let level_l = Self::fetch_sample(atari_ram, self.sample_ptr) as i32;
                let level_r = if stereo {
                    Self::fetch_sample(atari_ram, self.sample_ptr + 1) as i32
                } else {
                    level_l // Mono: duplicate to both channels
                };

                if b50k {
                    self.acc_50_l += level_l;
                    self.acc_50_r += level_r;
                    self.flip_50_to_25 = !self.flip_50_to_25;
                    if !self.flip_50_to_25 {
                        self.current_dac_level_l =
                            ((self.acc_50_l * self.master_volume) >> 1) as i16;
                        self.current_dac_level_r =
                            ((self.acc_50_r * self.master_volume) >> 1) as i16;
                        self.acc_50_l = 0;
                        self.acc_50_r = 0;
                    }
                } else {
                    self.current_dac_level_l = (level_l * self.master_volume) as i16;
                    self.current_dac_level_r = (level_r * self.master_volume) as i16;
                }

                self.sample_ptr += if stereo { 2 } else { 1 };
                self.inner_clock -= self.host_replay_rate;
            }
        } else {
            self.current_dac_level_l = 0;
            self.current_dac_level_r = 0;
        }
        // Apply muting
        let out_l = if self.mute_left { 0 } else { self.current_dac_level_l };
        let out_r = if self.mute_right { 0 } else { self.current_dac_level_r };
        // Store for visualization (before muting, to show actual DAC activity)
        self.last_output_l = self.current_dac_level_l;
        self.last_output_r = self.current_dac_level_r;
        (out_l, out_r)
    }

    /// Emulate internal rol to please any user 68k code reading & waiting the complete cycle
    fn microwire_tick(&mut self) -> u16 {
        if self.microwire_shift > 0 {
            self.microwire_mask = self.microwire_mask.rotate_left(1);
            self.microwire_shift -= 1;
        }
        self.microwire_mask
    }

    fn microwire_proceed(&mut self) {
        let mut value: u16 = 0;
        let mut count = 0;

        for i in 0..16 {
            if (self.microwire_mask & (1 << i)) != 0 {
                if (self.microwire_data & (1 << i)) != 0 {
                    value |= 1 << count;
                }
                count += 1;
            }
        }

        if count == 11 && (value >> 9) == 2 {
            let data = (value & 0x3f) as i32;
            if (value >> 6) & 7 == 3 {
                self.master_volume = if data > 40 { 64 } else { (data * 64) / 40 };
            }
        }
    }
}

impl Default for SteDac {
    fn default() -> Self {
        Self::new(44100)
    }
}
