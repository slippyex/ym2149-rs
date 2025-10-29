//! Cycle-Accurate Timing
//!
//! Tracks CPU/PSG cycle count for cycle-accurate emulation.

/// Cycle Counter for Cycle-Accurate Emulation
#[derive(Debug, Clone, Copy)]
pub struct CycleCounter {
    /// Current cycle count
    cycles: u64,
}

impl CycleCounter {
    /// Create a new cycle counter
    pub fn new() -> Self {
        CycleCounter { cycles: 0 }
    }

    /// Increment the cycle counter by one
    pub fn clock(&mut self) {
        self.cycles += 1;
    }

    /// Increment by n cycles
    pub fn advance(&mut self, n: u64) {
        self.cycles += n;
    }

    /// Get current cycle count
    pub fn get_cycles(&self) -> u64 {
        self.cycles
    }

    /// Reset the cycle counter
    pub fn reset(&mut self) {
        self.cycles = 0;
    }

    /// Get cycle count and reset
    pub fn take_and_reset(&mut self) -> u64 {
        let result = self.cycles;
        self.cycles = 0;
        result
    }
}

impl Default for CycleCounter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cycle_counter() {
        let mut counter = CycleCounter::new();
        assert_eq!(counter.get_cycles(), 0);

        counter.clock();
        assert_eq!(counter.get_cycles(), 1);

        counter.advance(99);
        assert_eq!(counter.get_cycles(), 100);
    }
}
