//! Shared helper utilities for YM2149 register math.
//!
//! These functions are used by downstream crates (CLI, Bevy plugin, visualization)
//! to derive channel periods and frequencies in a consistent way.

/// Default YM2149 master clock frequency used on the Atari ST (in Hz).
pub const PSG_MASTER_CLOCK_HZ: f32 = 2_000_000.0;

const PERIOD_DENOMINATOR: f32 = 16.0;

/// Compute the 12-bit tone period from register low/high bytes.
#[inline]
pub fn channel_period(lo: u8, hi: u8) -> Option<u16> {
    let period = (((hi as u16) & 0x0F) << 8) | (lo as u16);
    if period == 0 { None } else { Some(period) }
}

/// Convert a tone period into a frequency using the default 2MHz master clock.
#[inline]
pub fn period_to_frequency(period: u16) -> f32 {
    period_to_frequency_with_clock(PSG_MASTER_CLOCK_HZ, period)
}

/// Convert a tone period into a frequency for a specific master clock.
#[inline]
pub fn period_to_frequency_with_clock(master_clock_hz: f32, period: u16) -> f32 {
    if period == 0 {
        0.0
    } else {
        master_clock_hz / (PERIOD_DENOMINATOR * period as f32)
    }
}

/// Convenience helper returning the three channel frequencies for the default clock.
#[inline]
pub fn channel_frequencies(registers: &[u8; 16]) -> [Option<f32>; 3] {
    channel_frequencies_with_clock(registers, PSG_MASTER_CLOCK_HZ)
}

/// Compute the frequency of each channel for a given master clock.
#[inline]
pub fn channel_frequencies_with_clock(
    registers: &[u8; 16],
    master_clock_hz: f32,
) -> [Option<f32>; 3] {
    [
        channel_period(registers[0], registers[1])
            .map(|period| period_to_frequency_with_clock(master_clock_hz, period)),
        channel_period(registers[2], registers[3])
            .map(|period| period_to_frequency_with_clock(master_clock_hz, period)),
        channel_period(registers[4], registers[5])
            .map(|period| period_to_frequency_with_clock(master_clock_hz, period)),
    ]
}
