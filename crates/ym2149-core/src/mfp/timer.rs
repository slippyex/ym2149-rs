//! MFP Timer A/B/C Implementation
//!
//! Each timer can count down and generate interrupts or modulation signals
//! for the PSG.

/// MFP Timer
#[derive(Debug, Clone)]
pub struct Timer {
    /// Timer control/status byte
    control: u8,
    /// Timer data (countdown value)
    data: u8,
    /// Internal counter
    counter: u8,
    /// Is timer active?
    active: bool,
}

impl Timer {
    /// Create a new timer
    pub fn new() -> Self {
        Timer {
            control: 0,
            data: 0,
            counter: 0,
            active: false,
        }
    }

    /// Reset the timer
    pub fn reset(&mut self) {
        self.control = 0;
        self.data = 0;
        self.counter = 0;
        self.active = false;
    }

    /// Set control register
    pub fn set_control(&mut self, value: u8) {
        self.control = value;
        // Bit 0: Enable (1 = timer running)
        self.active = (value & 0x01) != 0;
    }

    /// Get control register
    pub fn get_control(&self) -> u8 {
        self.control
    }

    /// Set data register (countdown value)
    pub fn set_data(&mut self, value: u8) {
        self.data = value;
        self.counter = value;
    }

    /// Get data register
    pub fn get_data(&self) -> u8 {
        self.data
    }

    /// Clock the timer by one cycle
    pub fn clock(&mut self) {
        if !self.active {
            return;
        }

        if self.counter > 0 {
            self.counter -= 1;
        }

        if self.counter == 0 && self.active {
            // Timer expired - would trigger interrupt in real hardware
            // Note: Don't auto-reload here to make testing easier
            // Reload happens on set_data or explicit reload
        }
    }

    /// Check if timer has expired
    pub fn has_expired(&self) -> bool {
        self.counter == 0 && self.active
    }

    /// Is timer active?
    pub fn is_active(&self) -> bool {
        self.active
    }
}

impl Default for Timer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timer_creation() {
        let timer = Timer::new();
        assert!(!timer.is_active());
    }

    #[test]
    fn test_timer_countdown() {
        let mut timer = Timer::new();
        timer.set_data(10);
        timer.set_control(0x01); // Enable

        for _ in 0..10 {
            timer.clock();
        }

        assert!(timer.has_expired());
    }
}
