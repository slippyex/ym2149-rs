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
//! ## Register Map (accent addresses only, accent = odd byte)
//!
//! | Offset | Register | Description |
//! |--------|----------|-------------|
//! | 0x01   | GPIP     | General Purpose I/O |
//! | 0x03   | AER      | Active Edge Register |
//! | 0x05   | DDR      | Data Direction Register |
//! | 0x07   | IERA     | Interrupt Enable Register A |
//! | 0x09   | IERB     | Interrupt Enable Register B |
//! | 0x0B   | IPRA     | Interrupt Pending Register A |
//! | 0x0D   | IPRB     | Interrupt Pending Register B |
//! | 0x0F   | ISRA     | Interrupt In-Service Register A |
//! | 0x11   | ISRB     | Interrupt In-Service Register B |
//! | 0x13   | IMRA     | Interrupt Mask Register A |
//! | 0x15   | IMRB     | Interrupt Mask Register B |
//! | 0x17   | VR       | Vector Register |
//! | 0x19   | TACR     | Timer A Control Register |
//! | 0x1B   | TBCR     | Timer B Control Register |
//! | 0x1D   | TCDCR    | Timer C/D Control Register |
//! | 0x1F   | TADR     | Timer A Data Register |
//! | 0x21   | TBDR     | Timer B Data Register |
//! | 0x23   | TCDR     | Timer C Data Register |
//! | 0x25   | TDDR     | Timer D Data Register |
//!
//! ## Interrupt Priority (highest to lowest)
//!
//! Register A (IERA/IPRA/ISRA/IMRA):
//! - Bit 7: GPI7 (Mono detect on Atari ST)
//! - Bit 6: RS232 Receive error
//! - Bit 5: Timer A
//! - Bit 4: RS232 Receive buffer full
//! - Bit 3: RS232 Transmit error
//! - Bit 2: RS232 Transmit buffer empty
//! - Bit 1: GPI6
//! - Bit 0: Timer B
//!
//! Register B (IERB/IPRB/ISRB/IMRB):
//! - Bit 7: GPI5
//! - Bit 6: GPI4 (Keyboard/MIDI)
//! - Bit 5: Timer C
//! - Bit 4: Timer D
//! - Bit 3: GPI3 (Blitter)
//! - Bit 2: GPI2
//! - Bit 1: GPI1
//! - Bit 0: GPI0 (Centronics busy)
//!
//! The MFP is mapped at 0xFFFA00-0xFFFA25 on the Atari ST.

use ym2149_common::ATARI_MFP_CLOCK_HZ;

/// Timer prescaler values (MFP clock divided by prescaler)
const PRESCALE: [u32; 8] = [
    0,
    ATARI_MFP_CLOCK_HZ / 4,
    ATARI_MFP_CLOCK_HZ / 10,
    ATARI_MFP_CLOCK_HZ / 16,
    ATARI_MFP_CLOCK_HZ / 50,
    ATARI_MFP_CLOCK_HZ / 64,
    ATARI_MFP_CLOCK_HZ / 100,
    ATARI_MFP_CLOCK_HZ / 200,
];

// MFP Register offsets (accent addresses - accent = odd bytes only)
const REG_GPIP: usize = 0x01;
const REG_AER: usize = 0x03;
const REG_DDR: usize = 0x05;
const REG_IERA: usize = 0x07;
const REG_IERB: usize = 0x09;
const REG_IPRA: usize = 0x0B;
const REG_IPRB: usize = 0x0D;
const REG_ISRA: usize = 0x0F;
const REG_ISRB: usize = 0x11;
const REG_IMRA: usize = 0x13;
const REG_IMRB: usize = 0x15;
const REG_VR: usize = 0x17;
const REG_TACR: usize = 0x19;
const REG_TBCR: usize = 0x1B;
const REG_TCDCR: usize = 0x1D;
const REG_TADR: usize = 0x1F;
const REG_TBDR: usize = 0x21;
const REG_TCDR: usize = 0x23;
const REG_TDDR: usize = 0x25;

// Interrupt bit positions in IERA/IPRA/ISRA/IMRA
const INT_GPI7: u8 = 7;
const INT_TIMER_A: u8 = 5;
const INT_TIMER_B: u8 = 0;

// Interrupt bit positions in IERB/IPRB/ISRB/IMRB
const INT_TIMER_C: u8 = 5;
const INT_TIMER_D: u8 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerId {
    TimerA = 0,
    TimerB = 1,
    TimerC = 2,
    TimerD = 3,
    Gpi7 = 4,
}

/// CPU cycles per MFP clock tick (8 MHz CPU / 2.4576 MHz MFP ≈ 3.255)
/// We use fixed-point math: multiply by 256 for precision
const CPU_CYCLES_PER_MFP_TICK_FP8: u64 = (8_000_000 * 256) / ATARI_MFP_CLOCK_HZ as u64;

/// MFP prescaler divisors (index by control register bits 0-2)
const PRESCALER_DIV: [u32; 8] = [0, 4, 10, 16, 50, 64, 100, 200];

#[derive(Default)]
struct Timer {
    // === CONFIGURATION (stable, only changed by register writes) ===
    enable: bool,
    mask: bool,
    control_register: u8,
    data_register_init: u8, // Configured value (TxDR at CR write)

    // === LEGACY RUNTIME (only modified by tick()) ===
    inner_clock: u32,    // Sample accumulator
    legacy_counter: u8,  // Countdown counter for legacy mode (renamed from data_register)
    external_event: bool,
    last_input_state: bool, // Last input pin state for edge detection

    // === CYCLE-ACCURATE RUNTIME (independent from legacy) ===
    cycles_until_fire: Option<u64>, // RELATIVE, not absolute!
    last_check_cycle: u64,          // Last CPU cycle at check

    // === INTERRUPT STATE ===
    pending: bool,
    in_service: bool,
}

impl Timer {
    fn reset(&mut self) {
        // Configuration
        self.control_register = 0;
        self.data_register_init = 0;
        self.enable = false;
        self.mask = false;

        // Legacy runtime
        self.inner_clock = 0;
        self.legacy_counter = 0;
        self.external_event = false;
        self.last_input_state = false;

        // Cycle-accurate runtime
        self.cycles_until_fire = None;
        self.last_check_cycle = 0;

        // Interrupt state
        self.pending = false;
        self.in_service = false;
    }

    fn restart(&mut self) {
        // Reset legacy state
        self.inner_clock = 0;
        self.legacy_counter = self.data_register_init;

        // Reset cycle-accurate state
        self.cycles_until_fire = self.calc_cycles_for_period();
    }

    fn is_counter_mode(&self) -> bool {
        (self.control_register & 7) != 0 && (self.control_register & 8) == 0
    }

    fn is_event_mode(&self) -> bool {
        (self.control_register & 8) != 0
    }

    /// Calculate the number of CPU cycles for one full timer period.
    /// Returns None if timer is disabled or in event mode.
    /// Uses data_register_init (configuration), not legacy_counter.
    fn calc_cycles_for_period(&self) -> Option<u64> {
        if !self.enable || !self.is_counter_mode() {
            return None;
        }

        let prescaler = PRESCALER_DIV[(self.control_register & 7) as usize];
        if prescaler == 0 {
            return None;
        }

        // Timer counts down from data_register_init to 0, then fires
        // Number of MFP ticks for one period = data_register_init * prescaler
        let mfp_ticks = self.data_register_init as u64 * prescaler as u64;

        // Convert MFP ticks to CPU cycles using fixed-point math
        // cpu_cycles = mfp_ticks * (8MHz / 2.4576MHz) = mfp_ticks * 3.255...
        Some((mfp_ticks * CPU_CYCLES_PER_MFP_TICK_FP8) >> 8)
    }

    /// Reset cycle-accurate timer state after seek.
    /// Only resets cycle-accurate state from configuration (data_register_init).
    /// Legacy state (legacy_counter, inner_clock) remains untouched.
    fn reset_for_sync(&mut self, current_cpu_cycle: u64) {
        // Calculate cycles_until_fire from configuration (data_register_init)
        self.cycles_until_fire = self.calc_cycles_for_period();
        self.last_check_cycle = current_cpu_cycle;
        // Legacy state (legacy_counter, inner_clock) is NOT modified!
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

    /// Set input pin state with edge detection.
    /// Returns true if an event was triggered based on edge and AER setting.
    /// `active_edge_high`: true = rising edge triggers, false = falling edge triggers (from AER)
    fn set_input(&mut self, input: bool, active_edge_high: bool) -> bool {
        let last = self.last_input_state;
        self.last_input_state = input;

        let edge_detected = if active_edge_high {
            // Rising edge (0 -> 1) triggers
            !last && input
        } else {
            // Falling edge (1 -> 0) triggers
            last && !input
        };

        if edge_detected && self.is_event_mode() {
            self.external_event = true;
        }

        edge_detected
    }

    /// Tick timer (legacy sample-based mode).
    /// Only modifies legacy_counter and inner_clock - does NOT touch cycles_until_fire.
    fn tick(&mut self, host_replay_rate: u32) -> bool {
        let mut fired = false;

        if self.enable {
            if self.is_event_mode() {
                // Event mode - count on external events
                if self.external_event {
                    self.legacy_counter = self.legacy_counter.wrapping_sub(1);
                    if self.legacy_counter == 0 {
                        self.legacy_counter = self.data_register_init;
                        fired = true;
                    }
                    self.external_event = false;
                }
            } else if (self.control_register & 7) != 0 {
                // Timer counter mode
                self.inner_clock += PRESCALE[(self.control_register & 7) as usize];

                // Most of the time this while will never loop
                while self.inner_clock >= host_replay_rate {
                    self.legacy_counter = self.legacy_counter.wrapping_sub(1);
                    if self.legacy_counter == 0 {
                        self.legacy_counter = self.data_register_init;
                        fired = true;
                    }
                    self.inner_clock -= host_replay_rate;
                }
            }
        }

        // Set pending bit when timer fires
        if fired {
            self.pending = true;
        }

        // Return true only if masked and pending
        fired && self.mask
    }

    /// Check if this timer should fire at the given CPU cycle (cycle-accurate mode).
    /// Uses delta-based tracking with last_check_cycle - independent from legacy state.
    /// Returns true if the timer fired and interrupt should be dispatched.
    fn check_fire_at_cycle(&mut self, cpu_cycle: u64) -> bool {
        let elapsed = cpu_cycle.saturating_sub(self.last_check_cycle);
        self.last_check_cycle = cpu_cycle;

        if let Some(remaining) = self.cycles_until_fire {
            if elapsed >= remaining {
                // Timer fires
                self.pending = true;
                // Reload from configuration (not legacy state!)
                self.cycles_until_fire = self.calc_cycles_for_period();
                return self.mask; // Return true only if masked (interrupt enabled)
            } else {
                self.cycles_until_fire = Some(remaining - elapsed);
            }
        }
        false
    }

    /// Acknowledge interrupt - sets in_service, clears pending.
    fn acknowledge(&mut self) {
        if self.pending {
            self.pending = false;
            self.in_service = true;
        }
    }

    /// End of interrupt (for software EOI mode).
    fn end_of_interrupt(&mut self) {
        self.in_service = false;
    }
}

/// MFP68901 (MC68901) Multi-Function Peripheral emulation
pub struct Mfp68901 {
    host_replay_rate: u32,
    regs: [u8; 256],
    timers: [Timer; 5],
    /// GPIP - General Purpose I/O register (directly readable/writable)
    gpip: u8,
    /// AER - Active Edge Register (0 = falling edge, 1 = rising edge)
    aer: u8,
    /// DDR - Data Direction Register (0 = input, 1 = output)
    ddr: u8,
    /// VR - Vector Register (bit 3 = S bit for software EOI)
    vr: u8,
}

impl Mfp68901 {
    pub fn new(host_replay_rate: u32) -> Self {
        let mut mfp = Self {
            host_replay_rate,
            regs: [0; 256],
            timers: Default::default(),
            gpip: 0,
            aer: 0,
            ddr: 0,
            vr: 0,
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

        // Reset GPIO registers
        self.gpip = 0;
        self.aer = 0;
        self.ddr = 0;
        self.vr = 0;

        // By default on Atari OS timer C is enabled (and even running, but we just enable)
        self.timers[TimerId::TimerC as usize].enable = true;
        self.timers[TimerId::TimerC as usize].mask = true;

        // gpi7 is not really a timer, "simulate" an event type timer with count=1 to make the code simpler
        self.timers[TimerId::Gpi7 as usize].control_register = 1 << 3; // simulate event mode
        self.timers[TimerId::Gpi7 as usize].data_register_init = 1; // event count always 1
        self.timers[TimerId::Gpi7 as usize].legacy_counter = 1;
    }

    /// Check if software end-of-interrupt mode is enabled (S bit in VR).
    fn is_software_eoi(&self) -> bool {
        (self.vr & 0x08) != 0
    }

    /// Build IPRA from timer pending bits.
    fn build_ipra(&self) -> u8 {
        let mut ipra = 0u8;
        if self.timers[TimerId::Gpi7 as usize].pending {
            ipra |= 1 << INT_GPI7;
        }
        if self.timers[TimerId::TimerA as usize].pending {
            ipra |= 1 << INT_TIMER_A;
        }
        if self.timers[TimerId::TimerB as usize].pending {
            ipra |= 1 << INT_TIMER_B;
        }
        ipra
    }

    /// Build IPRB from timer pending bits.
    fn build_iprb(&self) -> u8 {
        let mut iprb = 0u8;
        if self.timers[TimerId::TimerC as usize].pending {
            iprb |= 1 << INT_TIMER_C;
        }
        if self.timers[TimerId::TimerD as usize].pending {
            iprb |= 1 << INT_TIMER_D;
        }
        iprb
    }

    /// Build ISRA from timer in-service bits.
    fn build_isra(&self) -> u8 {
        let mut isra = 0u8;
        if self.timers[TimerId::Gpi7 as usize].in_service {
            isra |= 1 << INT_GPI7;
        }
        if self.timers[TimerId::TimerA as usize].in_service {
            isra |= 1 << INT_TIMER_A;
        }
        if self.timers[TimerId::TimerB as usize].in_service {
            isra |= 1 << INT_TIMER_B;
        }
        isra
    }

    /// Build ISRB from timer in-service bits.
    fn build_isrb(&self) -> u8 {
        let mut isrb = 0u8;
        if self.timers[TimerId::TimerC as usize].in_service {
            isrb |= 1 << INT_TIMER_C;
        }
        if self.timers[TimerId::TimerD as usize].in_service {
            isrb |= 1 << INT_TIMER_D;
        }
        isrb
    }

    /// Apply IPRA write - writing 0 clears pending bits.
    fn apply_ipra(&mut self, data: u8) {
        // Writing 0 to a bit clears the pending state
        if (data & (1 << INT_GPI7)) == 0 {
            self.timers[TimerId::Gpi7 as usize].pending = false;
        }
        if (data & (1 << INT_TIMER_A)) == 0 {
            self.timers[TimerId::TimerA as usize].pending = false;
        }
        if (data & (1 << INT_TIMER_B)) == 0 {
            self.timers[TimerId::TimerB as usize].pending = false;
        }
    }

    /// Apply IPRB write - writing 0 clears pending bits.
    fn apply_iprb(&mut self, data: u8) {
        if (data & (1 << INT_TIMER_C)) == 0 {
            self.timers[TimerId::TimerC as usize].pending = false;
        }
        if (data & (1 << INT_TIMER_D)) == 0 {
            self.timers[TimerId::TimerD as usize].pending = false;
        }
    }

    /// Apply ISRA write - writing 0 clears in-service bits (software EOI).
    fn apply_isra(&mut self, data: u8) {
        if (data & (1 << INT_GPI7)) == 0 {
            self.timers[TimerId::Gpi7 as usize].in_service = false;
        }
        if (data & (1 << INT_TIMER_A)) == 0 {
            self.timers[TimerId::TimerA as usize].in_service = false;
        }
        if (data & (1 << INT_TIMER_B)) == 0 {
            self.timers[TimerId::TimerB as usize].in_service = false;
        }
    }

    /// Apply ISRB write - writing 0 clears in-service bits (software EOI).
    fn apply_isrb(&mut self, data: u8) {
        if (data & (1 << INT_TIMER_C)) == 0 {
            self.timers[TimerId::TimerC as usize].in_service = false;
        }
        if (data & (1 << INT_TIMER_D)) == 0 {
            self.timers[TimerId::TimerD as usize].in_service = false;
        }
    }

    /// Acknowledge interrupt for a specific timer.
    /// Called when the interrupt handler is entered.
    pub fn acknowledge_timer(&mut self, timer: TimerId) {
        self.timers[timer as usize].acknowledge();
    }

    /// End of interrupt for a timer (automatic EOI mode).
    /// Called when the interrupt handler returns.
    pub fn end_of_interrupt_timer(&mut self, timer: TimerId) {
        if !self.is_software_eoi() {
            self.timers[timer as usize].end_of_interrupt();
        }
    }

    pub fn write8(&mut self, port: u8, data: u8) {
        let port = port as usize & 255;

        if (port & 1) != 0 {
            match port {
                REG_GPIP => {
                    // Only output bits (DDR=1) can be written
                    self.gpip = (self.gpip & !self.ddr) | (data & self.ddr);
                }
                REG_AER => {
                    self.aer = data;
                }
                REG_DDR => {
                    self.ddr = data;
                }
                REG_IERA => {
                    self.timers[TimerId::TimerA as usize].set_er((data & (1 << INT_TIMER_A)) != 0);
                    self.timers[TimerId::TimerB as usize].set_er((data & (1 << INT_TIMER_B)) != 0);
                    self.timers[TimerId::Gpi7 as usize].set_er((data & (1 << INT_GPI7)) != 0);
                }
                REG_IERB => {
                    self.timers[TimerId::TimerC as usize].set_er((data & (1 << INT_TIMER_C)) != 0);
                    self.timers[TimerId::TimerD as usize].set_er((data & (1 << INT_TIMER_D)) != 0);
                }
                REG_IPRA => {
                    self.apply_ipra(data);
                }
                REG_IPRB => {
                    self.apply_iprb(data);
                }
                REG_ISRA => {
                    self.apply_isra(data);
                }
                REG_ISRB => {
                    self.apply_isrb(data);
                }
                REG_IMRA => {
                    self.timers[TimerId::TimerA as usize].set_mr((data & (1 << INT_TIMER_A)) != 0);
                    self.timers[TimerId::TimerB as usize].set_mr((data & (1 << INT_TIMER_B)) != 0);
                    self.timers[TimerId::Gpi7 as usize].set_mr((data & (1 << INT_GPI7)) != 0);
                }
                REG_IMRB => {
                    self.timers[TimerId::TimerC as usize].set_mr((data & (1 << INT_TIMER_C)) != 0);
                    self.timers[TimerId::TimerD as usize].set_mr((data & (1 << INT_TIMER_D)) != 0);
                }
                REG_VR => {
                    self.vr = data;
                }
                REG_TACR => {
                    self.timers[TimerId::TimerA as usize].set_cr(data & 0x0f);
                }
                REG_TBCR => {
                    self.timers[TimerId::TimerB as usize].set_cr(data & 0x0f);
                }
                REG_TCDCR => {
                    self.timers[TimerId::TimerC as usize].set_cr((data >> 4) & 7);
                    self.timers[TimerId::TimerD as usize].set_cr(data & 7);
                }
                REG_TADR | REG_TBDR | REG_TCDR | REG_TDDR => {
                    let timer_id = (port - REG_TADR) >> 1;
                    self.timers[timer_id].set_dr(data);
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
                REG_GPIP => {
                    // Return input pins (DDR=0) from external state, output pins from gpip
                    // For now, simulate mono detect (GPI7) as high (stereo mode)
                    data = (self.gpip & self.ddr) | (0x80 & !self.ddr);
                }
                REG_AER => {
                    data = self.aer;
                }
                REG_DDR => {
                    data = self.ddr;
                }
                REG_IPRA => {
                    data = self.build_ipra();
                }
                REG_IPRB => {
                    data = self.build_iprb();
                }
                REG_ISRA => {
                    data = self.build_isra();
                }
                REG_ISRB => {
                    data = self.build_isrb();
                }
                REG_VR => {
                    data = self.vr;
                }
                REG_TADR | REG_TBDR | REG_TCDR | REG_TDDR => {
                    let timer_id = (port - REG_TADR) >> 1;
                    // Return legacy_counter for hardware compatibility
                    // (some SNDH drivers read TxDR back to check current count)
                    data = self.timers[timer_id].legacy_counter;
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

    /// Tick all timers (legacy sample-based mode). Returns array indicating which timers fired.
    pub fn tick(&mut self) -> [bool; 5] {
        let mut fired = [false; 5];
        for (i, timer) in self.timers.iter_mut().enumerate() {
            fired[i] = timer.tick(self.host_replay_rate);
        }
        fired
    }

    /// Initialize cycle-accurate timer mode with current CPU cycle.
    /// Call this after reset and when timers are configured.
    pub fn sync_cpu_cycle(&mut self, cpu_cycle: u64) {
        for timer in &mut self.timers[0..4] {
            // Reset timer state and recalculate next fire cycle
            // This ensures clean state after seek
            timer.reset_for_sync(cpu_cycle);
        }
    }

    /// Check all timers at the given CPU cycle (cycle-accurate mode).
    /// Returns the TimerId of the highest-priority timer that fired, if any.
    /// Priority order: Timer A > Timer B > Timer C > Timer D
    pub fn check_timers_at_cycle(&mut self, cpu_cycle: u64) -> Option<TimerId> {
        // Check in priority order (A has highest priority among timers)
        // Note: GPI7 is not cycle-ticked, it's event-based
        if self.timers[TimerId::TimerA as usize].check_fire_at_cycle(cpu_cycle) {
            return Some(TimerId::TimerA);
        }
        if self.timers[TimerId::TimerB as usize].check_fire_at_cycle(cpu_cycle) {
            return Some(TimerId::TimerB);
        }
        if self.timers[TimerId::TimerC as usize].check_fire_at_cycle(cpu_cycle) {
            return Some(TimerId::TimerC);
        }
        if self.timers[TimerId::TimerD as usize].check_fire_at_cycle(cpu_cycle) {
            return Some(TimerId::TimerD);
        }
        None
    }

    /// Get the next CPU cycle at which any timer will fire.
    /// Returns None if no timers are active.
    pub fn next_timer_fire_cycle(&self) -> Option<u64> {
        self.timers[0..4]
            .iter()
            .filter_map(|t| {
                // Convert relative cycles_until_fire to absolute cycle
                t.cycles_until_fire.map(|remaining| t.last_check_cycle + remaining)
            })
            .min()
    }


    /// Set Timer A input pin state (TAI) with edge detection.
    /// Used for external event counting in Timer A event mode.
    pub fn set_timer_a_input(&mut self, state: bool) {
        // Timer A uses bit 4 of AER for edge selection (directly mapped to TAI)
        // Note: On real hardware, TAI has its own edge logic, but we use AER bit 4
        // as a reasonable approximation since it controls the same interrupt channel
        let active_edge_high = (self.aer & (1 << 4)) != 0;
        self.timers[TimerId::TimerA as usize].set_input(state, active_edge_high);
    }

    /// Set Timer B input pin state (TBI) with edge detection.
    /// Used for HBL counting and external event counting.
    #[allow(dead_code)] // Part of complete MFP API, used for HBL counting
    pub fn set_timer_b_input(&mut self, state: bool) {
        // Timer B uses bit 3 of AER for edge selection
        let active_edge_high = (self.aer & (1 << 3)) != 0;
        self.timers[TimerId::TimerB as usize].set_input(state, active_edge_high);
    }

    /// Set GPI7 input pin state (Mono detect on Atari ST) with edge detection.
    pub fn set_gpi7_input(&mut self, state: bool) {
        // GPI7 uses bit 7 of AER for edge selection
        let active_edge_high = (self.aer & (1 << 7)) != 0;
        self.timers[TimerId::Gpi7 as usize].set_input(state, active_edge_high);
    }

    /// Legacy method for STE DAC external event triggering.
    /// Pulses Timer A and GPI7 inputs high then low to trigger edge detection.
    pub fn set_ste_dac_external_event(&mut self) {
        // Simulate a pulse: high then low
        // This ensures edge detection works regardless of AER setting
        // (either rising or falling edge will be detected)
        self.set_timer_a_input(true);
        self.set_timer_a_input(false);
        self.set_gpi7_input(true);
        self.set_gpi7_input(false);
    }
}

impl Default for Mfp68901 {
    fn default() -> Self {
        Self::new(44100)
    }
}
