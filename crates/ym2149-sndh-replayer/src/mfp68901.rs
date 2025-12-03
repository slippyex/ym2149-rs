//! MFP68901 (MC68901) Multi-Function Peripheral timer emulation.
//!
//! The MFP68901 is a versatile peripheral chip used in the Atari ST for:
//! - Four programmable timers (A, B, C, D)
//! - Interrupt management
//! - Serial I/O (not emulated here)
//!
//! In SNDH replayers, the MFP timers are commonly used for:
//! - SID voice emulation (high-frequency timer interrupts)
//! - Sample playback timing
//! - Special effects (arpeggio, vibrato, etc.)
//!
//! ## Timer Modes
//!
//! Each timer can operate in:
//! - **Counter mode**: Counts down at a prescaled clock rate
//! - **Event mode**: Counts external events (e.g., STE DAC triggers)
//!
//! ## Memory Map
//!
//! The MFP is mapped at 0xFFFA00-0xFFFA25 on the Atari ST.

/// MFP clock frequency (2.4576 MHz).
const ATARI_MFP_CLOCK: u32 = 2457600;

/// Timer prescaler values (MFP clock divided by prescaler)
const PRESCALE: [u32; 8] = [
    0,
    ATARI_MFP_CLOCK / 4,
    ATARI_MFP_CLOCK / 10,
    ATARI_MFP_CLOCK / 16,
    ATARI_MFP_CLOCK / 50,
    ATARI_MFP_CLOCK / 64,
    ATARI_MFP_CLOCK / 100,
    ATARI_MFP_CLOCK / 200,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerId {
    TimerA = 0,
    TimerB = 1,
    TimerC = 2,
    TimerD = 3,
    Gpi7 = 4,
}

#[derive(Default)]
struct Timer {
    enable: bool,
    mask: bool,
    control_register: u8,
    data_register: u8,
    data_register_init: u8,
    inner_clock: u32,
    external_event: bool,
}

impl Timer {
    fn reset(&mut self) {
        self.control_register = 0;
        self.data_register = 0;
        self.enable = false;
        self.mask = false;
        self.inner_clock = 0;
        self.external_event = false;
    }

    fn restart(&mut self) {
        self.inner_clock = 0;
        self.data_register = self.data_register_init;
    }

    fn is_counter_mode(&self) -> bool {
        (self.control_register & 7) != 0 && (self.control_register & 8) == 0
    }

    fn set_er(&mut self, enable: bool) {
        if (self.enable ^ enable) && enable && self.is_counter_mode() {
            self.restart();
        }
        self.enable = enable;
    }

    fn set_dr(&mut self, data: u8) {
        self.data_register_init = data;
        if self.control_register == 0 {
            self.restart();
        }
    }

    fn set_cr(&mut self, data: u8) {
        self.control_register = data;
    }

    fn set_mr(&mut self, mask: bool) {
        self.mask = mask;
    }

    fn tick(&mut self, host_replay_rate: u32) -> bool {
        let mut ret = false;

        if self.enable {
            if (self.control_register & (1 << 3)) != 0 {
                // Event mode
                if self.external_event {
                    self.data_register = self.data_register.wrapping_sub(1);
                    if self.data_register == 0 {
                        self.data_register = self.data_register_init;
                        ret = true;
                    }
                    self.external_event = false;
                }
            } else if (self.control_register & 7) != 0 {
                // Timer counter mode
                self.inner_clock += PRESCALE[(self.control_register & 7) as usize];

                // Most of the time this while will never loop
                while self.inner_clock >= host_replay_rate {
                    self.data_register = self.data_register.wrapping_sub(1);
                    if self.data_register == 0 {
                        self.data_register = self.data_register_init;
                        ret = true;
                    }
                    self.inner_clock -= host_replay_rate;
                }
            }
        }

        ret && self.mask
    }
}

/// MFP68901 (MC68901) Multi-Function Peripheral emulation
pub struct Mfp68901 {
    host_replay_rate: u32,
    regs: [u8; 256],
    timers: [Timer; 5],
}

impl Mfp68901 {
    pub fn new(host_replay_rate: u32) -> Self {
        let mut mfp = Self {
            host_replay_rate,
            regs: [0; 256],
            timers: Default::default(),
        };
        mfp.reset();
        mfp
    }

    pub fn reset(&mut self) {
        for i in 0..256 {
            self.regs[i] = 0;
        }

        for t in 0..5 {
            self.timers[t].reset();
        }

        // By default on Atari OS timer C is enabled (and even running, but we just enable)
        self.timers[TimerId::TimerC as usize].enable = true;
        self.timers[TimerId::TimerC as usize].mask = true;

        // gpi7 is not really a timer, "simulate" an event type timer with count=1 to make the code simpler
        self.timers[TimerId::Gpi7 as usize].control_register = 1 << 3; // simulate event mode
        self.timers[TimerId::Gpi7 as usize].data_register_init = 1; // event count always 1
        self.timers[TimerId::Gpi7 as usize].data_register = 1;
    }

    pub fn write8(&mut self, port: u8, data: u8) {
        let port = port as usize & 255;

        if (port & 1) != 0 {
            match port {
                0x19 => {
                    self.timers[TimerId::TimerA as usize].set_cr(data & 0x0f);
                }
                0x1b => {
                    self.timers[TimerId::TimerB as usize].set_cr(data & 0x0f);
                }
                0x1d => {
                    self.timers[TimerId::TimerC as usize].set_cr((data >> 4) & 7);
                    self.timers[TimerId::TimerD as usize].set_cr(data & 7);
                }
                0x1f | 0x21 | 0x23 | 0x25 => {
                    let timer_id = (port - 0x1f) >> 1;
                    self.timers[timer_id].set_dr(data);
                }
                0x07 => {
                    self.timers[TimerId::TimerA as usize].set_er((data & (1 << 5)) != 0);
                    self.timers[TimerId::TimerB as usize].set_er((data & (1 << 0)) != 0);
                    self.timers[TimerId::Gpi7 as usize].set_er((data & (1 << 7)) != 0);
                }
                0x09 => {
                    self.timers[TimerId::TimerC as usize].set_er((data & (1 << 5)) != 0);
                    self.timers[TimerId::TimerD as usize].set_er((data & (1 << 4)) != 0);
                }
                0x13 => {
                    self.timers[TimerId::TimerA as usize].set_mr((data & (1 << 5)) != 0);
                    self.timers[TimerId::TimerB as usize].set_mr((data & (1 << 0)) != 0);
                    self.timers[TimerId::Gpi7 as usize].set_mr((data & (1 << 7)) != 0);
                }
                0x15 => {
                    self.timers[TimerId::TimerC as usize].set_mr((data & (1 << 5)) != 0);
                    self.timers[TimerId::TimerD as usize].set_mr((data & (1 << 4)) != 0);
                }
                _ => {}
            }
            self.regs[port] = data;
        }
    }

    pub fn read8(&self, port: u8) -> u8 {
        let port = port as usize & 255;
        let mut data = 0xff;

        if (port & 1) != 0 {
            data = self.regs[port];
            match port {
                0x01 => {
                    data = (self.regs[0x01] & 0x7f) | 0x80;
                }
                0x1f | 0x21 | 0x23 | 0x25 => {
                    let timer_id = (port - 0x1f) >> 1;
                    data = self.timers[timer_id].data_register;
                }
                _ => {}
            }
        }

        data
    }

    pub fn read16(&self, port: u8) -> u16 {
        0xff00 | self.read8(port + 1) as u16
    }

    pub fn write16(&mut self, port: u8, data: u16) {
        self.write8(port + 1, data as u8);
    }

    /// Tick all timers. Returns array indicating which timers fired.
    pub fn tick(&mut self) -> [bool; 5] {
        let mut fired = [false; 5];
        for (i, timer) in self.timers.iter_mut().enumerate() {
            fired[i] = timer.tick(self.host_replay_rate);
        }
        fired
    }

    pub fn set_ste_dac_external_event(&mut self) {
        self.timers[TimerId::TimerA as usize].external_event = true;
        self.timers[TimerId::Gpi7 as usize].external_event = true;
    }
}

impl Default for Mfp68901 {
    fn default() -> Self {
        Self::new(44100)
    }
}
