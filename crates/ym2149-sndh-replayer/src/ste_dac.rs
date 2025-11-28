//! Minimal STE DAC emulation (FF8900 range).
//!
//! Mirrors the behaviour used by the reference C++ player:
//! - DMA sample playback with loop handling
//! - Microwire master volume
//! - Stereo/mono modes and 50 kHz downmix to 25 kHz
//! - External event for MFP when the buffer loops

use crate::mfp68901::Mfp68901;

const STE_DAC_FREQ: u32 = 50_066;

pub struct SteDac {
    regs: [u8; 256],
    host_rate: u32,
    sample_ptr: u32,
    sample_end_ptr: u32,
    inner_clock: u32,
    microwire_mask: u16,
    microwire_shift: u16,
    microwire_data: u16,
    master_volume: i32,
    current_level: i32,
    acc_50: i32,
    toggle_50: bool,
}

impl SteDac {
    pub fn new(host_rate: u32) -> Self {
        let mut dac = Self {
            regs: [0; 256],
            host_rate,
            sample_ptr: 0,
            sample_end_ptr: 0,
            inner_clock: 0,
            microwire_mask: 0,
            microwire_shift: 0,
            microwire_data: 0,
            master_volume: 64,
            current_level: 0,
            acc_50: 0,
            toggle_50: false,
        };
        dac.reset(host_rate);
        dac
    }

    pub fn reset(&mut self, host_rate: u32) {
        self.regs.fill(0);
        self.host_rate = host_rate;
        self.sample_ptr = 0;
        self.sample_end_ptr = 0;
        self.inner_clock = 0;
        self.microwire_mask = 0;
        self.microwire_shift = 0;
        self.microwire_data = 0;
        self.master_volume = 64;
        self.current_level = 0;
        self.acc_50 = 0;
        self.toggle_50 = false;
    }

    fn fetch_sample_ptr(&mut self) {
        self.sample_ptr =
            (self.regs[3] as u32) << 16 | (self.regs[5] as u32) << 8 | (self.regs[7] as u32 & 0xfe);
        self.sample_end_ptr = (self.regs[0x0f] as u32) << 16
            | (self.regs[0x11] as u32) << 8
            | (self.regs[0x13] as u32 & 0xfe);
    }

    pub fn write8(&mut self, ad: u8, data: u8) {
        if ad & 1 == 0 {
            return;
        }
        let mut data = data;
        match ad {
            0x01 => {
                if (data & 1) != 0 && (data ^ self.regs[1]) & 1 != 0 {
                    // replay just started
                    self.fetch_sample_ptr();
                }
            }
            0x07 | 0x0d => data &= 0xfe,
            0x21 => {
                if (data & 3) != (self.regs[0x21] & 3) {
                    self.acc_50 = 0;
                    self.toggle_50 = false;
                }
            }
            _ => {}
        }
        self.regs[ad as usize] = data;
    }

    pub fn write16(&mut self, ad: u8, data: u16) {
        if ad & 1 != 0 {
            return;
        }
        match ad {
            0x22 => {
                self.microwire_data = data;
                self.microwire_proceed();
                self.microwire_shift = 16;
            }
            0x24 => {
                self.microwire_mask = data;
            }
            _ => self.write8(ad + 1, data as u8),
        }
    }

    pub fn read8(&self, ad: u8) -> u8 {
        if ad & 1 == 0 {
            return 0xff;
        }
        match ad {
            0x09 => (self.sample_ptr >> 16) as u8,
            0x0b => (self.sample_ptr >> 8) as u8,
            0x0d => self.sample_ptr as u8,
            _ => self.regs[ad as usize],
        }
    }

    pub fn read16(&mut self, ad: u8) -> u16 {
        if ad & 1 != 0 {
            return 0xffff;
        }
        match ad {
            0x22 => self.microwire_data,
            0x24 => self.microwire_tick(),
            _ => 0xff00 | self.read8(ad + 1) as u16,
        }
    }

    fn fetch_sample(&self, ram: &[u8], ad: u32) -> i16 {
        let idx = ad as usize;
        if idx < ram.len() {
            ram[idx] as i8 as i16
        } else {
            0
        }
    }

    pub fn compute_sample(&mut self, ram: &[u8], mfp: &mut Mfp68901) -> i16 {
        // Frequencies per Atari STE docs (approx)
        const DAC_FREQ: [u32; 4] = [
            STE_DAC_FREQ / 8,
            STE_DAC_FREQ / 4,
            STE_DAC_FREQ / 2,
            STE_DAC_FREQ,
        ];

        if (self.regs[1] & 1) != 0 {
            let rate_idx = (self.regs[0x21] & 3) as usize;
            let freq = DAC_FREQ[rate_idx];
            let stereo = (self.regs[0x21] & 0x80) == 0;
            let b50k = rate_idx == 3;

            self.inner_clock = self.inner_clock.saturating_add(freq);
            while self.inner_clock >= self.host_rate {
                if self.sample_ptr == self.sample_end_ptr {
                    mfp.set_ste_dac_external_event();
                    self.fetch_sample_ptr();
                    if (self.regs[0x1] & (1 << 1)) == 0 {
                        // No loop mode: stop playback
                        self.regs[0x1] &= 0xfe;
                        self.current_level = 0;
                        break;
                    }
                }

                let mut level = self.fetch_sample(ram, self.sample_ptr);
                if stereo {
                    level = level.saturating_add(self.fetch_sample(ram, self.sample_ptr + 1));
                }

                if b50k {
                    self.acc_50 += level as i32;
                    self.toggle_50 = !self.toggle_50;
                    if !self.toggle_50 {
                        self.current_level = (self.acc_50 * self.master_volume) >> 1;
                        self.acc_50 = 0;
                    }
                } else {
                    self.current_level = level as i32 * self.master_volume;
                }

                self.sample_ptr = self.sample_ptr.wrapping_add(if stereo { 2 } else { 1 });
                self.inner_clock -= self.host_rate;
            }
        } else {
            self.current_level = 0;
        }

        self.current_level.clamp(i16::MIN as i32, i16::MAX as i32) as i16
    }

    fn microwire_tick(&mut self) -> u16 {
        if self.microwire_shift > 0 {
            self.microwire_mask = self.microwire_mask.rotate_left(1);
            self.microwire_shift -= 1;
        }
        self.microwire_mask
    }

    fn microwire_proceed(&mut self) {
        let mut value = 0u16;
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
                self.master_volume = if data > 40 { 64 } else { data * 64 / 40 };
            }
        }
    }
}
