//! Fixed-point arithmetic for smooth slides and glides
//!
//! This module provides a fixed-point number type for precise sub-sample
//! calculations used in volume slides, pitch slides, and glide effects.
//! The C++ implementation uses a similar approach with FpFloat.

use std::ops::{Add, AddAssign, Sub, SubAssign};

/// Fixed-point number with 8-bit fractional part
///
/// Stores a value as integer part + fractional part (1/256ths).
/// This allows smooth slides without floating point arithmetic.
///
/// Example: Value 5.25 is stored as (5 << 8) + 64 = 1344
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FixedPoint {
    raw: i32, // Integer part in high bits, fractional in low 8 bits
}

impl FixedPoint {
    /// Number of fractional bits
    const FRAC_BITS: u32 = 8;
    const FRAC_MASK: i32 = (1 << Self::FRAC_BITS) - 1;

    /// Create from integer value
    #[inline]
    pub fn from_int(value: i32) -> Self {
        Self {
            raw: value << Self::FRAC_BITS,
        }
    }

    /// Create from raw value (for internal use)
    #[inline]
    pub const fn from_raw(raw: i32) -> Self {
        Self { raw }
    }

    /// Create from two hex digits (Arkos format)
    ///
    /// In Arkos effects, pitch/volume values are often given as two hex digits
    /// where the first digit is the integer part and second is fractional.
    /// Example: 0x34 means 3.25 (3 + 4/16)
    #[inline]
    pub fn from_digits(value: u16) -> Self {
        let integer_part = ((value >> 8) & 0xFF) as i32;
        let frac_part = (value & 0xFF) as i32;
        let raw = (integer_part << Self::FRAC_BITS) + frac_part;
        Self { raw }
    }

    /// Get integer part
    #[inline]
    pub fn integer_part(self) -> i32 {
        self.raw >> Self::FRAC_BITS
    }

    /// Get fractional part (0-255)
    #[inline]
    pub fn frac_part(self) -> u8 {
        (self.raw & Self::FRAC_MASK) as u8
    }

    /// Reset to zero
    #[inline]
    pub fn reset(&mut self) {
        self.raw = 0;
    }

    /// Set from integer value
    #[inline]
    pub fn set_int(&mut self, value: i32) {
        self.raw = value << Self::FRAC_BITS;
    }

    /// Set from raw value
    #[inline]
    pub fn set_raw(&mut self, raw: i32) {
        self.raw = raw;
    }

    /// Negate the value
    #[inline]
    pub fn negate(&mut self) {
        self.raw = -self.raw;
    }

    /// Get as u16 (for periods, clamped to 0..65535)
    #[inline]
    pub fn as_u16(self) -> u16 {
        self.integer_part().clamp(0, 65535) as u16
    }

    /// Clamp value between 0 and max (inclusive)
    #[inline]
    pub fn clamp(&mut self, max: i32) {
        let int_part = self.integer_part();
        if int_part < 0 {
            self.reset();
        } else if int_part > max {
            self.set_int(max);
        }
    }

    /// Get raw value (for serialization/debugging)
    #[inline]
    pub fn raw(self) -> i32 {
        self.raw
    }

    /// Multiply by an integer value
    #[inline]
    pub fn mul_int(self, value: i32) -> Self {
        Self {
            raw: self.raw.saturating_mul(value),
        }
    }
}

impl Add for FixedPoint {
    type Output = Self;

    #[inline]
    fn add(self, other: Self) -> Self {
        Self {
            raw: self.raw + other.raw,
        }
    }
}

impl AddAssign for FixedPoint {
    #[inline]
    fn add_assign(&mut self, other: Self) {
        self.raw += other.raw;
    }
}

impl Sub for FixedPoint {
    type Output = Self;

    #[inline]
    fn sub(self, other: Self) -> Self {
        Self {
            raw: self.raw - other.raw,
        }
    }
}

impl SubAssign for FixedPoint {
    #[inline]
    fn sub_assign(&mut self, other: Self) {
        self.raw -= other.raw;
    }
}

impl From<i32> for FixedPoint {
    #[inline]
    fn from(value: i32) -> Self {
        Self::from_int(value)
    }
}

impl From<u16> for FixedPoint {
    #[inline]
    fn from(value: u16) -> Self {
        Self::from_int(value as i32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_int() {
        let fp = FixedPoint::from_int(5);
        assert_eq!(fp.integer_part(), 5);
        assert_eq!(fp.frac_part(), 0);
    }

    #[test]
    fn test_from_digits() {
        // 0x0100 = 1.0
        let fp = FixedPoint::from_digits(0x0100);
        assert_eq!(fp.integer_part(), 1);
        assert_eq!(fp.frac_part(), 0);

        // 0x0208 = 2 + 8/256
        let fp2 = FixedPoint::from_digits(0x0208);
        assert_eq!(fp2.integer_part(), 2);
        assert_eq!(fp2.frac_part(), 8);
    }

    #[test]
    fn test_add() {
        let a = FixedPoint::from_int(5);
        let b = FixedPoint::from_digits(0x0112); // 1 + 0x12/256
        let c = a + b;
        assert_eq!(c.integer_part(), 6);
    }

    #[test]
    fn test_clamp() {
        let mut fp = FixedPoint::from_int(20);
        fp.clamp(15);
        assert_eq!(fp.integer_part(), 15);

        let mut fp2 = FixedPoint::from_int(-5);
        fp2.clamp(15);
        assert_eq!(fp2.integer_part(), 0);
    }

    #[test]
    fn test_negate() {
        let mut fp = FixedPoint::from_int(5);
        fp.negate();
        assert_eq!(fp.integer_part(), -5);
    }
}
