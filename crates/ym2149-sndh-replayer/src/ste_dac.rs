//! STE DAC emulation

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
    /// Accumulator for 50kHz mode
    acc_50: i32,
    current_dac_level: i16,
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
            acc_50: 0,
            current_dac_level: 0,
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
        self.current_dac_level = 0;
        self.acc_50 = 0;
        self.flip_50_to_25 = false;
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
                    }
                }
                0x07 | 0x0d => {
                    data &= 0xfe;
                }
                0x21 => {
                    if (data & 3) != (self.regs[0x21] & 3) {
                        self.acc_50 = 0;
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

    /// Compute next DAC sample
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
    pub fn compute_sample(&mut self, atari_ram: &[u8], mfp: &mut Mfp68901) -> i16 {
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
                        self.current_dac_level = 0;
                        break;
                    }
                }

                let mut level = Self::fetch_sample(atari_ram, self.sample_ptr) as i32;
                if stereo {
                    level += Self::fetch_sample(atari_ram, self.sample_ptr + 1) as i32;
                }

                if b50k {
                    self.acc_50 += level;
                    self.flip_50_to_25 = !self.flip_50_to_25;
                    if !self.flip_50_to_25 {
                        self.current_dac_level = ((self.acc_50 * self.master_volume) >> 1) as i16;
                        self.acc_50 = 0;
                    }
                } else {
                    self.current_dac_level = (level * self.master_volume) as i16;
                }

                self.sample_ptr += if stereo { 2 } else { 1 };
                self.inner_clock -= self.host_replay_rate;
            }
        } else {
            self.current_dac_level = 0;
        }
        self.current_dac_level
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
