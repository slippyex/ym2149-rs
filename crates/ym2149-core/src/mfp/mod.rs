//! MFP (Multi-Function Peripheral) Integration
//!
//! ATARI ST MFP timer infrastructure for PSG modulation.
//!
//! ## Effects
//!
//! YM6 special effects (SID Voice, Sync Buzzer, DigiDrum, Sinus SID) are decoded
//! by the YM format parser (`crate::ym_parser::effects`) since they are part of
//! the YM file format specification, not MFP-specific infrastructure.

pub mod timer;

pub use timer::Timer;

/// MFP Multi-Function Peripheral
///
/// The MFP provides three timers (A, B, C) that can interact with the PSG
/// for various modulation effects on ATARI ST.
#[derive(Debug, Clone)]
pub struct Mfp {
    /// Timer A
    pub timer_a: Timer,
    /// Timer B
    pub timer_b: Timer,
    /// Timer C
    pub timer_c: Timer,
}

impl Mfp {
    /// Create a new MFP instance
    pub fn new() -> Self {
        Mfp {
            timer_a: Timer::new(),
            timer_b: Timer::new(),
            timer_c: Timer::new(),
        }
    }

    /// Reset MFP to initial state
    pub fn reset(&mut self) {
        self.timer_a.reset();
        self.timer_b.reset();
        self.timer_c.reset();
    }

    /// Clock the MFP by one cycle
    pub fn clock(&mut self) {
        self.timer_a.clock();
        self.timer_b.clock();
        self.timer_c.clock();
    }
}

impl Default for Mfp {
    fn default() -> Self {
        Self::new()
    }
}
