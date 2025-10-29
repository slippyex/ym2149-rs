//! YM2149 Register Definitions
//!
//! Defines the 16 registers (R0-R13, R14-R15 for I/O ports) that control
//! the PSG chip. Each register controls specific aspects of sound generation.

use std::fmt;

/// YM2149 Register Address
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Register {
    /// Channel A Frequency (low byte) - R0
    ChAFreqLo = 0x00,
    /// Channel A Frequency (high byte) - R1
    ChAFreqHi = 0x01,
    /// Channel B Frequency (low byte) - R2
    ChBFreqLo = 0x02,
    /// Channel B Frequency (high byte) - R3
    ChBFreqHi = 0x03,
    /// Channel C Frequency (low byte) - R4
    ChCFreqLo = 0x04,
    /// Channel C Frequency (high byte) - R5
    ChCFreqHi = 0x05,
    /// Noise Frequency Control - R6
    NoiseFreq = 0x06,
    /// Mixer Control (enable/disable channels and noise) - R7
    MixerCtrl = 0x07,
    /// Channel A Amplitude - R8
    ChAAmplitude = 0x08,
    /// Channel B Amplitude - R9
    ChBAmplitude = 0x09,
    /// Channel C Amplitude - R10
    ChCAmplitude = 0x0A,
    /// Envelope Frequency (low byte) - R11
    EnvelopeFreqLo = 0x0B,
    /// Envelope Frequency (high byte) - R12
    EnvelopeFreqHi = 0x0C,
    /// Envelope Shape - R13
    EnvelopeShape = 0x0D,
    /// I/O Port A - R14
    PortA = 0x0E,
    /// I/O Port B - R15
    PortB = 0x0F,
}

impl Register {
    /// Convert a raw register number (0-15) to Register enum
    pub fn from_addr(addr: u8) -> Option<Self> {
        match addr & 0x0F {
            0x00 => Some(Register::ChAFreqLo),
            0x01 => Some(Register::ChAFreqHi),
            0x02 => Some(Register::ChBFreqLo),
            0x03 => Some(Register::ChBFreqHi),
            0x04 => Some(Register::ChCFreqLo),
            0x05 => Some(Register::ChCFreqHi),
            0x06 => Some(Register::NoiseFreq),
            0x07 => Some(Register::MixerCtrl),
            0x08 => Some(Register::ChAAmplitude),
            0x09 => Some(Register::ChBAmplitude),
            0x0A => Some(Register::ChCAmplitude),
            0x0B => Some(Register::EnvelopeFreqLo),
            0x0C => Some(Register::EnvelopeFreqHi),
            0x0D => Some(Register::EnvelopeShape),
            0x0E => Some(Register::PortA),
            0x0F => Some(Register::PortB),
            _ => None,
        }
    }

    /// Get the register address value
    pub fn addr(&self) -> u8 {
        *self as u8
    }
}

impl fmt::Display for Register {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Register::ChAFreqLo => write!(f, "R0 (Channel A Frequency Low)"),
            Register::ChAFreqHi => write!(f, "R1 (Channel A Frequency High)"),
            Register::ChBFreqLo => write!(f, "R2 (Channel B Frequency Low)"),
            Register::ChBFreqHi => write!(f, "R3 (Channel B Frequency High)"),
            Register::ChCFreqLo => write!(f, "R4 (Channel C Frequency Low)"),
            Register::ChCFreqHi => write!(f, "R5 (Channel C Frequency High)"),
            Register::NoiseFreq => write!(f, "R6 (Noise Frequency)"),
            Register::MixerCtrl => write!(f, "R7 (Mixer Control)"),
            Register::ChAAmplitude => write!(f, "R8 (Channel A Amplitude)"),
            Register::ChBAmplitude => write!(f, "R9 (Channel B Amplitude)"),
            Register::ChCAmplitude => write!(f, "R10 (Channel C Amplitude)"),
            Register::EnvelopeFreqLo => write!(f, "R11 (Envelope Frequency Low)"),
            Register::EnvelopeFreqHi => write!(f, "R12 (Envelope Frequency High)"),
            Register::EnvelopeShape => write!(f, "R13 (Envelope Shape)"),
            Register::PortA => write!(f, "R14 (I/O Port A)"),
            Register::PortB => write!(f, "R15 (I/O Port B)"),
        }
    }
}

/// Raw register bank (16 bytes)
#[derive(Debug, Clone, Copy)]
pub struct RegisterBank {
    /// Register values R0-R15
    pub registers: [u8; 16],
}

impl RegisterBank {
    /// Create a new register bank with all values set to 0
    pub fn new() -> Self {
        RegisterBank { registers: [0; 16] }
    }

    /// Read a register value
    pub fn read(&self, addr: u8) -> u8 {
        self.registers[(addr & 0x0F) as usize]
    }

    /// Write a register value
    pub fn write(&mut self, addr: u8, value: u8) {
        self.registers[(addr & 0x0F) as usize] = value;
    }

    /// Get all registers as a slice
    pub fn as_slice(&self) -> &[u8; 16] {
        &self.registers
    }
}

impl Default for RegisterBank {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_conversion() {
        assert_eq!(Register::from_addr(0x00), Some(Register::ChAFreqLo));
        assert_eq!(Register::from_addr(0x0D), Some(Register::EnvelopeShape));
        assert_eq!(Register::from_addr(0x0F), Some(Register::PortB));
        assert_eq!(Register::from_addr(0x10), Some(Register::ChAFreqLo)); // Should wrap
    }

    #[test]
    fn test_register_bank() {
        let mut bank = RegisterBank::new();
        assert_eq!(bank.read(0x00), 0);

        bank.write(0x00, 0x42);
        assert_eq!(bank.read(0x00), 0x42);
    }
}
