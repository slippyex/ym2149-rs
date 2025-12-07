//! Spectrum analyzer display widget.
//!
//! Displays chromatic frequency bars (one per semitone) using Ratatui's BarChart.
//! Supports up to 12 channels (4 PSGs × 3 channels) for multi-PSG configurations.
//! Each bar shows the maximum value across all channels, colored by the dominant channel.
//! Bar brightness is modulated by velocity (rate of change) for dynamic visualization.

use super::App;
use ratatui::{
    Frame,
    prelude::*,
    style::Color,
    widgets::{Bar, BarChart, BarGroup, Block, Borders},
};
use ym2149_common::visualization::SPECTRUM_BINS;

/// Base RGB colors for each channel (will be brightened by velocity)
/// PSG 0: Red, Green, Blue
/// PSG 1: Yellow, Cyan, Magenta
/// PSG 2: Orange-Red, Lime-Green, Sky-Blue
/// PSG 3: Gold, Teal, Pink
const CHANNEL_BASE_RGB: [(u8, u8, u8); 12] = [
    // PSG 0 - Primary colors
    (180, 60, 60), // Red
    (60, 180, 60), // Green
    (60, 60, 180), // Blue
    // PSG 1 - Secondary colors
    (180, 180, 60), // Yellow
    (60, 180, 180), // Cyan
    (180, 60, 180), // Magenta
    // PSG 2 - Bright variants
    (200, 100, 60), // Orange-Red
    (100, 200, 60), // Lime-Green
    (60, 150, 200), // Sky-Blue
    // PSG 3 - More variants
    (200, 180, 60),  // Gold
    (60, 150, 150),  // Teal
    (200, 100, 150), // Pink
];

/// Brighten a color based on velocity (0.0-1.0)
fn brighten_color(base: (u8, u8, u8), velocity: f32) -> Color {
    // Velocity boosts brightness: low velocity = dim, high velocity = bright
    let boost = 1.0 + velocity * 1.5; // Up to 2.5x brightness
    let r = ((base.0 as f32 * boost).min(255.0)) as u8;
    let g = ((base.1 as f32 * boost).min(255.0)) as u8;
    let b = ((base.2 as f32 * boost).min(255.0)) as u8;
    Color::Rgb(r, g, b)
}

/// Blend multiple channel colors based on their contribution
fn blend_channel_colors(contributions: &[(usize, f32, f32)]) -> Color {
    if contributions.is_empty() {
        return Color::DarkGray;
    }

    // Sum weighted colors
    let mut r_sum = 0.0f32;
    let mut g_sum = 0.0f32;
    let mut b_sum = 0.0f32;
    let mut weight_sum = 0.0f32;
    let mut max_velocity = 0.0f32;

    for &(ch, value, velocity) in contributions {
        if value > 0.01 {
            let base = CHANNEL_BASE_RGB[ch % 12];
            r_sum += base.0 as f32 * value;
            g_sum += base.1 as f32 * value;
            b_sum += base.2 as f32 * value;
            weight_sum += value;
            max_velocity = max_velocity.max(velocity);
        }
    }

    if weight_sum < 0.01 {
        return Color::DarkGray;
    }

    // Normalize and apply velocity brightness
    let boost = 1.0 + max_velocity * 1.5;
    let r = ((r_sum / weight_sum * boost).min(255.0)) as u8;
    let g = ((g_sum / weight_sum * boost).min(255.0)) as u8;
    let b = ((b_sum / weight_sum * boost).min(255.0)) as u8;

    Color::Rgb(r, g, b)
}

/// Draw spectrum analyzer with chromatic (semitone) resolution
pub fn draw_spectrum(f: &mut Frame, area: Rect, app: &App) {
    // Get per-channel spectrum data, velocity, and effect status for all active channels
    let capture = app.capture.lock();
    let channel_count = capture.channel_count();
    let spectrums: Vec<_> = (0..channel_count)
        .map(|ch| *capture.spectrum_channel(ch))
        .collect();
    let sid_active: Vec<bool> = (0..channel_count)
        .map(|ch| capture.is_sid_active(ch))
        .collect();
    let drum_active: Vec<bool> = (0..channel_count)
        .map(|ch| capture.is_drum_active(ch))
        .collect();
    // Get velocity for each channel/bin
    let velocities: Vec<Vec<f32>> = (0..channel_count)
        .map(|ch| {
            (0..SPECTRUM_BINS)
                .map(|bin| capture.spectrum_velocity(ch, bin))
                .collect()
        })
        .collect();
    drop(capture);

    if spectrums.is_empty() {
        return;
    }

    // Create one bar per bin (semitone), combining all channels
    let mut bars: Vec<Bar> = Vec::with_capacity(SPECTRUM_BINS);

    for bin_idx in 0..SPECTRUM_BINS {
        // Collect contributions from all channels for this bin
        let mut contributions: Vec<(usize, f32, f32)> = Vec::new();
        let mut max_value: f32 = 0.0;
        let mut has_drum = false;
        let mut has_sid = false;

        for (ch_idx, spectrum) in spectrums.iter().enumerate() {
            let is_drum = drum_active.get(ch_idx).copied().unwrap_or(false);
            let is_sid = sid_active.get(ch_idx).copied().unwrap_or(false);
            let velocity = velocities
                .get(ch_idx)
                .and_then(|v| v.get(bin_idx))
                .copied()
                .unwrap_or(0.0);

            let value = if is_drum {
                has_drum = true;
                // Drums: broadband noise across mid-high bins (scaled to 32 bins)
                // Center around bin 18 (~C5-C6), spread across bins 12-28
                if (12..=28).contains(&bin_idx) {
                    let center_dist = ((bin_idx as f32 - 18.0).abs() / 8.0).min(1.0);
                    0.8 * (1.0 - center_dist * 0.5)
                } else if bin_idx > 28 {
                    0.3
                } else {
                    0.1
                }
            } else if is_sid {
                has_sid = true;
                spectrum[bin_idx]
            } else {
                spectrum[bin_idx]
            };

            if value > 0.01 {
                contributions.push((ch_idx, value, velocity));
                max_value = max_value.max(value);
            }
        }

        // Determine color based on channel contributions
        let color = if has_drum && max_value > 0.01 {
            // Drum: white with velocity-based brightness
            let max_vel = contributions.iter().map(|c| c.2).fold(0.0f32, f32::max);
            let brightness = (200.0 + max_vel * 55.0).min(255.0) as u8;
            Color::Rgb(brightness, brightness, brightness)
        } else if has_sid && max_value > 0.01 {
            // SID: cyan with velocity brightness
            let max_vel = contributions.iter().map(|c| c.2).fold(0.0f32, f32::max);
            let base_brightness = 150.0 + max_vel * 105.0;
            Color::Rgb(
                (base_brightness * 0.3).min(255.0) as u8,
                base_brightness.min(255.0) as u8,
                base_brightness.min(255.0) as u8,
            )
        } else if contributions.len() == 1 {
            // Single channel: use its color with velocity
            let (ch, _, velocity) = contributions[0];
            brighten_color(CHANNEL_BASE_RGB[ch % 12], velocity)
        } else {
            // Multiple channels: blend colors
            blend_channel_colors(&contributions)
        };

        bars.push(
            Bar::default()
                .value((max_value * 100.0) as u64)
                .style(Style::default().fg(color))
                .text_value(String::new()),
        );
    }

    let bar_group = BarGroup::default().bars(&bars);

    let chart = BarChart::default()
        .block(Block::default().borders(Borders::ALL).title(" Spectrum "))
        .data(bar_group)
        .bar_width(1)
        .bar_gap(1)
        .max(100);

    f.render_widget(chart, area);
}
