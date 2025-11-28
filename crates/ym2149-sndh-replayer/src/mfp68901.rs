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
//!
//! Timer implementation ported from Leonard/Oxygene's AtariAudio.

/// Atari ST MFP clock frequency (2.4576 MHz)
const ATARI_MFP_CLOCK: u32 = 2_457_600;

/// Timer prescaler frequencies (MFP clock divided by prescaler)
/// This is the key insight from AtariAudio: pre-calculate the frequencies
/// instead of using raw divisor values with complex fixed-point math.
const PRESCALE: [u32; 8] = [
    0,
    ATARI_MFP_CLOCK / 4,   // = 614400
    ATARI_MFP_CLOCK / 10,  // = 245760
    ATARI_MFP_CLOCK / 16,  // = 153600
    ATARI_MFP_CLOCK / 50,  // = 49152
    ATARI_MFP_CLOCK / 64,  // = 38400
    ATARI_MFP_CLOCK / 100, // = 24576
    ATARI_MFP_CLOCK / 200, // = 12288
];

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
    /// Internal clock accumulator (simple u32 like AtariAudio)
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
        // Match reference Mk68901: enabling a counter-mode timer restarts it
        if (self.enable ^ enable) && enable && self.is_counter_mode() {
            self.restart();
        }
        self.enable = enable;
    }

    /// Set data register
    fn set_data(&mut self, data: u8) {
        self.data_init = data;
        // Reload only when control is zero (match C++ ref behaviour)
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
    /// Implementation ported from Leonard/Oxygene's AtariAudio.
    fn tick(&mut self, host_replay_rate: u32) -> bool {
        let mut ret = false;

        if self.enable {
            if (self.control & (1 << 3)) != 0 {
                // Event mode
                if self.external_event {
                    self.data = self.data.wrapping_sub(1);
                    if self.data == 0 {
                        self.data = self.data_init;
                        ret = true;
                    }
                    self.external_event = false;
                }
            } else if (self.control & 7) != 0 {
                // Timer counter mode - simple accumulator logic from AtariAudio
                self.inner_clock += PRESCALE[(self.control & 7) as usize];

                // Most of the time this while will never loop
                while self.inner_clock >= host_replay_rate {
                    self.data = self.data.wrapping_sub(1);
                    if self.data == 0 {
                        self.data = self.data_init;
                        ret = true;
                    }
                    self.inner_clock -= host_replay_rate;
                }
            }
        }

        ret && self.mask
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

    /// Tick all timers for one sample period.
    ///
    /// Returns which timers fired (index order: A, B, C, D, GPI7).
    pub fn tick(&mut self) -> [bool; 5] {
        let mut fired = [false; 5];
        for (idx, timer) in self.timers.iter_mut().enumerate() {
            fired[idx] = timer.tick(self.host_replay_rate);
        }

        fired
    }

    /// Write an 8-bit value to an MFP register.
    ///
    /// Port is relative to MFP base address (0xFFFA00).
    pub fn write8(&mut self, port: u8, data: u8) {
        // MFP registers are on odd addresses only
        if (port & 1) == 0 {
            return;
        }

        #[cfg(debug_assertions)]
        {
            if std::env::var_os("YM2149_MFP_DEBUG").is_some() {
                let reg_name = match port {
                    0x07 => "IER_A",
                    0x09 => "IER_B",
                    0x13 => "IMR_A",
                    0x15 => "IMR_B",
                    0x19 => "TACR",
                    0x1B => "TBCR",
                    0x1D => "TCDCR",
                    0x1F => "TADR",
                    0x21 => "TBDR",
                    0x23 => "TCDR",
                    0x25 => "TDDR",
                    _ => "???",
                };
                eprintln!("MFP write ${:02X} ({}) = ${:02X}", port, reg_name, data);
            }
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
    pub fn read16(&self, port: u8) -> u16 {
        0xFF00 | self.read8(port.wrapping_add(1)) as u16
    }

    /// Set external event flag for STE DAC timers.
    pub fn set_ste_dac_external_event(&mut self) {
        self.timers[TimerId::TimerA as usize].external_event = true;
        self.timers[TimerId::Gpi7 as usize].external_event = true;
    }

    /// Force an external event on a specific timer (debug/testing).
    pub fn trigger_external_event(&mut self, timer: TimerId) {
        self.timers[timer as usize].external_event = true;
    }

    /// Debug helper to inspect a timer (read-only copy).
    pub fn debug_timer(&self, idx: usize) -> DebugTimer {
        let t = &self.timers[idx];
        DebugTimer {
            control: t.control,
            data: t.data,
            data_init: t.data_init,
            enable: t.enable,
            mask: t.mask,
        }
    }
}

/// Debug view of an MFP timer.
#[derive(Debug, Clone, Copy)]
pub struct DebugTimer {
    pub control: u8,
    pub data: u8,
    #[allow(dead_code)]
    pub data_init: u8,
    pub enable: bool,
    pub mask: bool,
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

    #[test]
    fn test_timer_b_fires() {
        let mut mfp = Mfp68901::new(44100);

        // Setup Timer B like SNDH files do for SID effects
        mfp.write8(0x1B, 0x01); // TBCR = prescaler /4
        mfp.write8(0x21, 0x4B); // TBDR = 75
        mfp.write8(0x07, 0x01); // IER_A = enable Timer B (bit 0)
        mfp.write8(0x13, 0x01); // IMR_A = mask Timer B (bit 0)

        let timer_b = &mfp.timers[TimerId::TimerB as usize];
        assert!(timer_b.enable);
        assert!(timer_b.mask);
        assert_eq!(timer_b.control, 0x01);
        assert_eq!(timer_b.data_init, 0x4B);

        // Timer B at prescaler /4 with data=75:
        // Timer frequency = 2457600 / 4 / 75 = 8192 Hz
        // At 44100 Hz sample rate, timer fires 8192/44100 = 0.1857 times per sample
        // In 44100 samples (1 second), timer should fire ~8192 times
        let samples = 44100;
        let mut fire_count = 0;
        for _ in 0..samples {
            let fired = mfp.tick();
            if fired[1] {
                fire_count += 1;
            }
        }

        // Expected: 8192 fires per second, allow 1% tolerance
        let expected = 8192;
        let tolerance = expected / 100; // 1%
        assert!(
            fire_count >= expected - tolerance && fire_count <= expected + tolerance,
            "Timer B fired {} times, expected {} ± {}",
            fire_count,
            expected,
            tolerance
        );
    }

    #[test]
    fn test_enable_restarts_like_mym_replayer() {
        let mut mfp = Mfp68901::new(44100);

        // Mimic MYM_REPL.BIN timer hook: clear control, write data, then set enable/mask
        mfp.write8(0x19, 0x00); // TACR = 0 (stops + primes reload)
        mfp.write8(0x1F, 0x10); // TADR = 0x10 -> reloads because control==0

        // Set prescaler /4, then enable+mask bit 5 (Timer A)
        mfp.write8(0x19, 0x01); // TACR = /4 (no restart yet)
        mfp.write8(0x07, 0x20); // IERA bit5 = enable (should restart)
        mfp.write8(0x13, 0x20); // IMRA bit5 = mask

        // After enable restart, timer should start from data_init (0x10) and fire after 0x10 decrements.
        let mut fires = 0;
        for _ in 0..2000 {
            if mfp.tick()[0] {
                fires += 1;
            }
        }

        // /4 prescale with data=16 -> 2457600/4/16 ≈ 38400 Hz, so ~1700 fires at 44.1kHz for 2000 samples.
        assert!(
            (1500..=1900).contains(&fires),
            "Timer A fires={}, expected around 1700",
            fires
        );
    }

    #[test]
    fn test_all_timers_fire() {
        let mut mfp = Mfp68901::new(44100);

        // Setup Timer A: prescaler /10, data=50 -> 2457600/10/50 = 4915.2 Hz
        mfp.write8(0x19, 0x02); // TACR = prescaler /10
        mfp.write8(0x1F, 50); // TADR = 50
        mfp.write8(0x07, 0x20); // IER_A = enable Timer A (bit 5)
        mfp.write8(0x13, 0x20); // IMR_A = mask Timer A (bit 5)

        // Setup Timer B: prescaler /4, data=75 -> 8192 Hz
        mfp.write8(0x1B, 0x01); // TBCR = prescaler /4
        mfp.write8(0x21, 75); // TBDR = 75
        mfp.write8(0x07, mfp.read8(0x07) | 0x01); // IER_A |= Timer B (bit 0)
        mfp.write8(0x13, mfp.read8(0x13) | 0x01); // IMR_A |= Timer B (bit 0)

        // Timer C is already enabled by default (from reset)
        // Setup Timer C: prescaler /64, data=192 -> 2457600/64/192 = 200 Hz
        mfp.write8(0x1D, 0x50); // TCDCR = Timer C prescaler /64 (bits 4-6)
        mfp.write8(0x23, 192); // TCDR = 192

        // Setup Timer D: prescaler /16, data=100 -> 2457600/16/100 = 1536 Hz
        mfp.write8(0x1D, mfp.read8(0x1D) | 0x03); // TCDCR |= Timer D prescaler /16 (bits 0-2)
        mfp.write8(0x25, 100); // TDDR = 100
        mfp.write8(0x09, 0x30); // IER_B = enable Timer C+D
        mfp.write8(0x15, 0x30); // IMR_B = mask Timer C+D

        let samples = 44100; // 1 second
        let mut fire_counts = [0u32; 5];
        for _ in 0..samples {
            let fired = mfp.tick();
            for (idx, f) in fired.iter().enumerate() {
                if *f {
                    fire_counts[idx] += 1;
                }
            }
        }

        // Verify all timers fire at approximately correct rates
        // Timer A: ~4915 Hz
        assert!(
            fire_counts[0] >= 4800 && fire_counts[0] <= 5000,
            "Timer A fired {} times, expected ~4915",
            fire_counts[0]
        );
        // Timer B: ~8192 Hz
        assert!(
            fire_counts[1] >= 8100 && fire_counts[1] <= 8300,
            "Timer B fired {} times, expected ~8192",
            fire_counts[1]
        );
        // Timer C: ~200 Hz
        assert!(
            fire_counts[2] >= 190 && fire_counts[2] <= 210,
            "Timer C fired {} times, expected ~200",
            fire_counts[2]
        );
        // Timer D: ~1536 Hz
        assert!(
            fire_counts[3] >= 1500 && fire_counts[3] <= 1600,
            "Timer D fired {} times, expected ~1536",
            fire_counts[3]
        );
    }

    #[test]
    fn test_data_zero_means_256() {
        // Test that data=0 is treated as period 256 (hardware behavior)
        let mut mfp = Mfp68901::new(44100);

        // Setup Timer A: prescaler /4, data=0 -> should be period 256
        // Timer frequency = 2457600 / 4 / 256 = 2400 Hz
        mfp.write8(0x19, 0x00); // TACR = 0 (stop, prime reload)
        mfp.write8(0x1F, 0x00); // TADR = 0 (means 256)
        mfp.write8(0x19, 0x01); // TACR = /4
        mfp.write8(0x07, 0x20); // IER_A = enable Timer A
        mfp.write8(0x13, 0x20); // IMR_A = mask Timer A

        let samples = 44100; // 1 second
        let mut fire_count = 0;
        for _ in 0..samples {
            if mfp.tick()[0] {
                fire_count += 1;
            }
        }

        // Expected: 2457600 / 4 / 256 = 2400 Hz, allow 2% tolerance
        let expected = 2400;
        let tolerance = expected / 50; // 2%
        assert!(
            fire_count >= expected - tolerance && fire_count <= expected + tolerance,
            "Timer A with data=0 fired {} times, expected {} ± {} (data=0 should mean period 256)",
            fire_count,
            expected,
            tolerance
        );
    }
}
