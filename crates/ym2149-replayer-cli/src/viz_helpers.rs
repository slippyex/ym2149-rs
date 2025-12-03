//! Terminal Visualization Utilities
//!
//! Utilities for creating visual feedback in terminal-based applications.
//! These tools help display real-time audio information in a user-friendly way.

use std::fmt::Write;

/// Create a compact channel status line showing register settings and active effects
///
/// Displays tone/noise enable flags, amplitude level, envelope effects, and SID/DigiDrum status
/// in a compact single-line format suitable for terminal display.
///
/// # Arguments
/// * `tone_enabled` - Whether the tone is enabled for this channel
/// * `noise_enabled` - Whether the noise is enabled for this channel
/// * `amplitude` - Amplitude level (0-15)
/// * `envelope_enabled` - Whether envelope modulation is active
/// * `envelope_shape` - Name of the active envelope shape effect (e.g., "SAWDN", "AD")
/// * `sid_active` - Whether SID voice effect is active on this channel
/// * `drum_active` - Whether DigiDrum effect is active on this channel
/// * `sync_buzzer_active` - Whether sync buzzer effect is active
///
/// # Returns
/// A formatted status string showing tone/noise state, amplitude, active effects and special effects
#[allow(clippy::too_many_arguments)]
pub fn create_channel_status(
    tone_enabled: bool,
    noise_enabled: bool,
    amplitude: u8,
    envelope_enabled: bool,
    envelope_shape: &str,
    sid_active: bool,
    drum_active: bool,
    sync_buzzer_active: bool,
) -> String {
    let mut status = String::with_capacity(48);

    // Tone indicator
    if tone_enabled {
        write!(status, "T").ok();
    } else {
        write!(status, "-").ok();
    }

    // Noise indicator
    if noise_enabled {
        write!(status, "N").ok();
    } else {
        write!(status, "-").ok();
    }

    // Amplitude level
    write!(
        status,
        " {}:{}",
        if envelope_enabled { "E" } else { "A" },
        amplitude
    )
    .ok();

    // Envelope effect indicator
    if envelope_enabled && !envelope_shape.is_empty() {
        write!(status, " {}", envelope_shape).ok();
    }

    // Special effects indicators
    let mut effects = Vec::new();
    if sid_active {
        effects.push("SID");
    }
    if drum_active {
        effects.push("DRUM");
    }
    if sync_buzzer_active {
        effects.push("SYNC");
    }

    if !effects.is_empty() {
        write!(status, " [{}]", effects.join("|")).ok();
    }

    status
}

/// Create a Unicode block bar representing an amplitude value
///
/// Generates a fixed-width string with █ characters proportional to the amplitude level,
/// padded with spaces to maintain consistent width. This is useful for creating visual
/// volume meters in terminal applications with proper alignment.
///
/// # Arguments
/// * `amplitude` - Amplitude value (0.0 to 1.0+, clamped internally)
/// * `max_length` - Maximum bar length in characters (also the fixed output width)
///
/// # Returns
/// Fixed-width string of █ characters padded with spaces
pub fn create_volume_bar(amplitude: f32, max_length: usize) -> String {
    let normalized = amplitude.clamp(0.0, 1.0);
    let block_count = (normalized * max_length as f32) as usize;
    let blocks = "█".repeat(block_count.min(max_length));
    let spaces = " ".repeat(max_length.saturating_sub(block_count));
    format!("{}{}", blocks, spaces)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_status_all_enabled() {
        let status = create_channel_status(true, true, 15, true, "SAWDN", false, false, false);
        assert!(status.contains("T"));
        assert!(status.contains("N"));
        assert!(status.contains("E:15"));
        assert!(status.contains("SAWDN"));
    }

    #[test]
    fn test_channel_status_tone_only() {
        let status = create_channel_status(true, false, 8, false, "", false, false, false);
        assert!(status.contains("T"));
        assert!(status.contains("-"));
        assert!(status.contains("A:8"));
    }

    #[test]
    fn test_channel_status_noise_only() {
        let status = create_channel_status(false, true, 5, false, "", false, false, false);
        assert!(status.contains("-"));
        assert!(status.contains("N"));
        assert!(status.contains("A:5"));
    }

    #[test]
    fn test_channel_status_disabled() {
        let status = create_channel_status(false, false, 0, false, "", false, false, false);
        assert!(status.contains("--"));
        assert!(status.contains("A:0"));
    }

    #[test]
    fn test_channel_status_with_effects() {
        let status = create_channel_status(true, false, 8, false, "", true, true, true);
        assert!(status.contains("T"));
        assert!(status.contains("SID"));
        assert!(status.contains("DRUM"));
        assert!(status.contains("SYNC"));
    }

    #[test]
    fn test_volume_bar_full() {
        let bar = create_volume_bar(1.0, 10);
        assert_eq!(bar.chars().count(), 10);
    }

    #[test]
    fn test_volume_bar_empty() {
        let bar = create_volume_bar(0.0, 10);
        assert_eq!(bar.chars().count(), 10); // Fixed width with spaces
        assert_eq!(bar.trim().len(), 0); // No blocks
    }

    #[test]
    fn test_volume_bar_half() {
        let bar = create_volume_bar(0.5, 10);
        assert_eq!(bar.chars().count(), 10); // Fixed width
        assert_eq!(bar.trim().chars().count(), 5); // 5 blocks
    }

    #[test]
    fn test_volume_bar_clamping() {
        let bar = create_volume_bar(1.5, 10);
        assert_eq!(bar.chars().count(), 10);
    }

    #[test]
    fn test_volume_bar_negative_clamping() {
        let bar = create_volume_bar(-0.5, 10);
        assert_eq!(bar.chars().count(), 10); // Fixed width
        assert_eq!(bar.trim().len(), 0); // No blocks (clamped to 0)
    }

    #[test]
    fn test_volume_bar_various_lengths() {
        for length in [1, 5, 10, 20] {
            let bar = create_volume_bar(1.0, length);
            assert_eq!(bar.chars().count(), length);
        }
    }
}
