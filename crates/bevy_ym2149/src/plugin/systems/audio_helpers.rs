//! Audio calculation helpers

const PSG_MASTER_CLOCK_HZ: f32 = 2_000_000.0;

/// Calculate channel period from register bytes
pub(super) fn channel_period(lo: u8, hi: u8) -> Option<u16> {
    let period = ((hi as u16) << 8) | (lo as u16);
    if period > 0 { Some(period) } else { None }
}

/// Convert period to frequency
pub(super) fn period_to_frequency(period: u16) -> f32 {
    PSG_MASTER_CLOCK_HZ / (16.0 * period as f32)
}

/// Get frequencies for all three channels
pub(super) fn channel_frequencies(registers: &[u8; 16]) -> [Option<f32>; 3] {
    [
        channel_period(registers[0], registers[1]).map(period_to_frequency),
        channel_period(registers[2], registers[3]).map(period_to_frequency),
        channel_period(registers[4], registers[5]).map(period_to_frequency),
    ]
}
