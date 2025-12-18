//! YM2149 Hardware Constants
//!
//! Shared constants and lookup tables used across PSG components.

/// YM2149 hardware volume table (normalized to 0.0-1.0)
///
/// The YM2149 uses a non-linear volume curve rather than simple linear scaling.
/// This table maps amplitude register values (0-15) to their actual output levels,
/// matching hardware measurements and datasheet specifications.
///
/// These values reflect the exponential nature of perceived loudness and the
/// hardware's internal digital-to-analog converter characteristics.
///
/// # Example
/// Amplitude level 7 produces approximately 1.6% output, not the 46.7% that
/// linear scaling would suggest. This matches human perception of loudness
/// (which is logarithmic, not linear).
pub const VOLUME_TABLE: [u16; 16] = [
    20, 53, 88, 125, 193, 258, 385, 525, 753, 1029, 1523, 2077, 3110, 4395, 7073, 10922,
];

/// Scale factor that maps STSound integer amplitudes to normalized floats.
pub const VOLUME_SCALE: f32 = 1.0 / 32767.0;

/// YM2149 envelope DAC table (32 steps, 5-bit resolution)
/// Get volume level for amplitude register value (0-15)
///
/// Masks the input to ensure it's in the valid range [0, 15] and
/// returns the corresponding volume from the VOLUME_TABLE.
///
/// # Arguments
/// * `amplitude` - Amplitude register value (any u8, will be masked to 0-15)
///
/// # Returns
/// Volume level (0.0 to 1.0)
#[inline]
pub fn get_volume(amplitude: u8) -> f32 {
    VOLUME_TABLE[(amplitude & 0x0F) as usize] as f32 * VOLUME_SCALE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_volume_table_edge_values() {
        // Test that edge values are within expected bounds
        assert!(get_volume(0) < 0.001, "Lowest level should be near silence");
        assert!(
            (get_volume(15) - (10922.0 / 32767.0)).abs() < 1e-6,
            "Max level should match ST reference"
        );
    }

    #[test]
    fn test_volume_table_monotonic_increasing() {
        // Test that volume table is monotonically increasing
        for i in 1..16 {
            assert!(
                VOLUME_TABLE[i] > VOLUME_TABLE[i - 1],
                "Volume table not monotonic: STSOUND_VOLUME_TABLE[{}] ({}) <= STSOUND_VOLUME_TABLE[{}] ({})",
                i,
                VOLUME_TABLE[i],
                i - 1,
                VOLUME_TABLE[i - 1]
            );
        }
    }

    #[test]
    fn test_volume_table_all_values_in_range() {
        // Test that all values are in valid range [0.0, 1.0]
        for amplitude in 0u8..=15u8 {
            let val = get_volume(amplitude);
            assert!(
                (0.0..=1.0).contains(&val),
                "Volume table value {val} at index {amplitude} out of range [0.0, 1.0]"
            );
        }
    }

    #[test]
    fn test_volume_table_size() {
        // Test that volume table has exactly 16 entries (for 0-15 amplitude levels)
        assert_eq!(VOLUME_TABLE.len(), 16);
    }

    #[test]
    fn test_get_volume_basic_values() {
        // Test get_volume() with basic values
        assert!(get_volume(0) < 0.001, "get_volume(0) should be near silent");
        assert!((get_volume(15) - (10922.0 / 32767.0)).abs() < 1e-6);
        assert!(
            get_volume(7) > 0.015 && get_volume(7) < 0.017,
            "get_volume(7) should be ~1.6%"
        );
    }

    #[test]
    fn test_get_volume_with_mask() {
        // Test that get_volume() correctly masks input to 0-15
        // Any value with bits 7-4 set should be masked off
        assert_eq!(
            get_volume(0x0F),
            get_volume(0xFF),
            "0xFF should mask to 0x0F"
        );
        assert_eq!(
            get_volume(0x07),
            get_volume(0x87),
            "0x87 should mask to 0x07"
        );
        assert_eq!(
            get_volume(0x00),
            get_volume(0xF0),
            "0xF0 should mask to 0x00"
        );
    }

    #[test]
    fn test_get_volume_all_values() {
        // Test that get_volume() returns the correct table value for all amplitudes
        for (amplitude, &raw) in VOLUME_TABLE.iter().enumerate() {
            let expected = raw as f32 * VOLUME_SCALE;
            let actual = get_volume(amplitude as u8);
            assert!((actual - expected).abs() < f32::EPSILON * 2.0);
        }
    }

    #[test]
    fn test_volume_table_exponential_progression() {
        // Test that volume table follows approximate exponential curve
        // Calculate average ratio between consecutive values (should be ~1.43)
        let mut ratios = Vec::new();
        for i in 1..VOLUME_TABLE.len() {
            let prev = VOLUME_TABLE[i - 1];
            if prev > 0 {
                ratios.push(VOLUME_TABLE[i] as f32 / prev as f32);
            }
        }
        let avg_ratio = ratios.iter().sum::<f32>() / ratios.len() as f32;
        assert!(avg_ratio > 1.2 && avg_ratio < 1.7);
    }
}
