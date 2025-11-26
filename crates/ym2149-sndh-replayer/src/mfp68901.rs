//! MFP 68901 (Multi-Function Peripheral) timer emulation.
//!
//! The MFP 68901 is used in the Atari ST for timers, serial communication,
//! and GPIO. For SNDH playback, we only need the timer functionality.
//!
//! The chip has 4 timers (A, B, C, D) plus a GPI7 interrupt. Each timer
//! can run in counter mode (prescaler-based) or event mode (external trigger).
//!
//! Timers A and B are used by many SNDH files for SID voice effects and
//! other timer-based sound techniques.

/// Atari ST MFP clock frequency (2.4576 MHz)
const ATARI_MFP_CLOCK: u32 = 2_457_600;

/// Timer prescaler values for counter mode (control register bits 0-2)
const PRESCALE_VALUES: [u32; 8] = [0, 4, 10, 16, 50, 64, 100, 200];

/// Timer identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerId {
    TimerA = 0,
    TimerB = 1,
    TimerC = 2,
    TimerD = 3,
    Gpi7 = 4,
}

/// Single MFP timer state
#[derive(Debug, Clone, Default)]
struct Timer {
    /// Timer enabled (IER bit)
    enable: bool,
    /// Interrupt mask (IMR bit)
    mask: bool,
    /// Control register value
    control: u8,
    /// Data register (current counter value)
    data: u8,
    /// Data register initial value (reload on timeout)
    data_init: u8,
    /// Internal clock accumulator
    inner_clock: u32,
    /// External event flag (for event mode)
    external_event: bool,
}

impl Timer {
    fn reset(&mut self) {
        self.enable = false;
        self.mask = false;
        self.control = 0;
        self.data = 0;
        self.data_init = 0;
        self.inner_clock = 0;
        self.external_event = false;
    }

    fn restart(&mut self) {
        self.inner_clock = 0;
        self.data = self.data_init;
    }

    /// Check if timer is in counter mode (not event mode)
    fn is_counter_mode(&self) -> bool {
        let ctrl = self.control & 0x0F;
        (ctrl & 7) != 0 && (ctrl & 8) == 0
    }

    /// Set enable register bit
    fn set_enable(&mut self, enable: bool) {
        if self.enable != enable && enable && self.is_counter_mode() {
            self.restart();
        }
        self.enable = enable;
    }

    /// Set data register
    fn set_data(&mut self, data: u8) {
        self.data_init = data;
        if self.control == 0 {
            self.restart();
        }
    }

    /// Set control register
    fn set_control(&mut self, control: u8) {
        self.control = control;
    }

    /// Set mask register bit
    fn set_mask(&mut self, mask: bool) {
        self.mask = mask;
    }

    /// Tick the timer by one sample period.
    ///
    /// Returns true if the timer fired (and interrupt is enabled+masked).
    fn tick(&mut self, host_replay_rate: u32) -> bool {
        if !self.enable {
            return false;
        }

        let ctrl = self.control & 0x0F;

        // Event mode (bit 3 set)
        if (ctrl & 8) != 0 {
            if self.external_event {
                self.data = self.data.wrapping_sub(1);
                if self.data == 0 {
                    self.data = self.data_init;
                    self.external_event = false;
                    return self.mask;
                }
                self.external_event = false;
            }
            return false;
        }

        // Counter mode
        let prescale_index = (ctrl & 7) as usize;
        if prescale_index == 0 {
            return false;
        }

        let prescale = ATARI_MFP_CLOCK / PRESCALE_VALUES[prescale_index];
        self.inner_clock += prescale;

        let mut fired = false;
        while self.inner_clock >= host_replay_rate {
            self.data = self.data.wrapping_sub(1);
            if self.data == 0 {
                self.data = self.data_init;
                fired = true;
            }
            self.inner_clock -= host_replay_rate;
        }

        fired && self.mask
    }
}

/// MFP 68901 emulation for Atari ST
#[derive(Debug, Clone)]
pub struct Mfp68901 {
    /// Timer states
    timers: [Timer; 5],
    /// Register mirror (for reading back)
    regs: [u8; 256],
    /// Host replay rate
    host_replay_rate: u32,
}

impl Default for Mfp68901 {
    fn default() -> Self {
        Self::new(44100)
    }
}

impl Mfp68901 {
    /// Create a new MFP with the given host sample rate.
    pub fn new(host_replay_rate: u32) -> Self {
        let mut mfp = Self {
            timers: Default::default(),
            regs: [0; 256],
            host_replay_rate,
        };
        mfp.reset();
        mfp
    }

    /// Reset all timers and registers.
    pub fn reset(&mut self) {
        for timer in &mut self.timers {
            timer.reset();
        }
        self.regs.fill(0);

        // By default on Atari OS, timer C is enabled
        self.timers[TimerId::TimerC as usize].enable = true;
        self.timers[TimerId::TimerC as usize].mask = true;

        // GPI7 simulates event mode with count=1
        let gpi7 = &mut self.timers[TimerId::Gpi7 as usize];
        gpi7.control = 1 << 3; // Event mode
        gpi7.data_init = 1;
        gpi7.data = 1;
    }

    /// Set the host replay rate.
    #[allow(dead_code)]
    pub fn set_host_rate(&mut self, rate: u32) {
        self.host_replay_rate = rate;
    }

    /// Write an 8-bit value to an MFP register.
    ///
    /// Port is relative to MFP base address (0xFFFA00).
    pub fn write8(&mut self, port: u8, data: u8) {
        // MFP registers are on odd addresses only
        if (port & 1) == 0 {
            return;
        }

        match port {
            0x19 => {
                // Timer A control
                self.timers[TimerId::TimerA as usize].set_control(data & 0x0F);
            }
            0x1B => {
                // Timer B control
                self.timers[TimerId::TimerB as usize].set_control(data & 0x0F);
            }
            0x1D => {
                // Timer C/D control (combined register)
                self.timers[TimerId::TimerC as usize].set_control((data >> 4) & 0x07);
                self.timers[TimerId::TimerD as usize].set_control(data & 0x07);
            }
            0x1F => {
                // Timer A data
                self.timers[TimerId::TimerA as usize].set_data(data);
            }
            0x21 => {
                // Timer B data
                self.timers[TimerId::TimerB as usize].set_data(data);
            }
            0x23 => {
                // Timer C data
                self.timers[TimerId::TimerC as usize].set_data(data);
            }
            0x25 => {
                // Timer D data
                self.timers[TimerId::TimerD as usize].set_data(data);
            }
            0x07 => {
                // IER A (Interrupt Enable Register A)
                self.timers[TimerId::TimerA as usize].set_enable((data & (1 << 5)) != 0);
                self.timers[TimerId::TimerB as usize].set_enable((data & (1 << 0)) != 0);
                self.timers[TimerId::Gpi7 as usize].set_enable((data & (1 << 7)) != 0);
            }
            0x09 => {
                // IER B (Interrupt Enable Register B)
                self.timers[TimerId::TimerC as usize].set_enable((data & (1 << 5)) != 0);
                self.timers[TimerId::TimerD as usize].set_enable((data & (1 << 4)) != 0);
            }
            0x13 => {
                // IMR A (Interrupt Mask Register A)
                self.timers[TimerId::TimerA as usize].set_mask((data & (1 << 5)) != 0);
                self.timers[TimerId::TimerB as usize].set_mask((data & (1 << 0)) != 0);
                self.timers[TimerId::Gpi7 as usize].set_mask((data & (1 << 7)) != 0);
            }
            0x15 => {
                // IMR B (Interrupt Mask Register B)
                self.timers[TimerId::TimerC as usize].set_mask((data & (1 << 5)) != 0);
                self.timers[TimerId::TimerD as usize].set_mask((data & (1 << 4)) != 0);
            }
            _ => {}
        }

        self.regs[port as usize] = data;
    }

    /// Read an 8-bit value from an MFP register.
    pub fn read8(&self, port: u8) -> u8 {
        // MFP registers are on odd addresses only
        if (port & 1) == 0 {
            return 0xFF;
        }

        match port {
            0x01 => {
                // GPIP: bit 7 always high (monochrome detect)
                (self.regs[0x01] & 0x7F) | 0x80
            }
            0x1F => self.timers[TimerId::TimerA as usize].data,
            0x21 => self.timers[TimerId::TimerB as usize].data,
            0x23 => self.timers[TimerId::TimerC as usize].data,
            0x25 => self.timers[TimerId::TimerD as usize].data,
            _ => self.regs[port as usize],
        }
    }

    /// Write a 16-bit value to MFP (writes to odd byte only).
    pub fn write16(&mut self, port: u8, data: u16) {
        self.write8(port.wrapping_add(1), data as u8);
    }

    /// Read a 16-bit value from MFP.
    #[allow(dead_code)]
    pub fn read16(&self, port: u8) -> u16 {
        0xFF00 | self.read8(port.wrapping_add(1)) as u16
    }

    /// Tick a specific timer by one sample.
    ///
    /// Returns true if the timer fired an interrupt.
    pub fn tick(&mut self, timer_id: TimerId) -> bool {
        self.timers[timer_id as usize].tick(self.host_replay_rate)
    }

    /// Set external event flag for STE DAC timers.
    #[allow(dead_code)]
    pub fn set_ste_dac_external_event(&mut self) {
        self.timers[TimerId::TimerA as usize].external_event = true;
        self.timers[TimerId::Gpi7 as usize].external_event = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mfp_reset() {
        let mfp = Mfp68901::new(44100);
        assert!(mfp.timers[TimerId::TimerC as usize].enable);
        assert!(mfp.timers[TimerId::TimerC as usize].mask);
    }

    #[test]
    fn test_timer_data_write_read() {
        let mut mfp = Mfp68901::new(44100);
        mfp.write8(0x1F, 100); // Timer A data
        assert_eq!(mfp.read8(0x1F), 100);
    }

    #[test]
    fn test_timer_control() {
        let mut mfp = Mfp68901::new(44100);
        mfp.write8(0x19, 0x07); // Timer A control = prescale /200
        assert_eq!(mfp.timers[TimerId::TimerA as usize].control, 0x07);
    }
}
