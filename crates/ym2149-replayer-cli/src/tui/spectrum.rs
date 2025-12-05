//! Spectrum analyzer display widget.
//!
//! Displays frequency bars per channel using Ratatui's BarChart widget.
//! Supports up to 12 channels (4 PSGs × 3 channels) for multi-PSG configurations.

use super::App;
use ratatui::{
    Frame,
    prelude::*,
    style::Color,
    widgets::{Bar, BarChart, BarGroup, Block, Borders},
};

/// Channel colors matching the oscilloscope
/// PSG 0: Red, Green, Blue
/// PSG 1: Yellow, Cyan, Magenta
/// PSG 2: LightRed, LightGreen, LightBlue
/// PSG 3: LightYellow, LightCyan, LightMagenta
const CHANNEL_COLORS: [Color; 12] = [
    // PSG 0
    Color::Red,
    Color::Green,
    Color::Blue,
    // PSG 1
    Color::Yellow,
    Color::Cyan,
    Color::Magenta,
    // PSG 2
    Color::LightRed,
    Color::LightGreen,
    Color::LightBlue,
    // PSG 3
    Color::LightYellow,
    Color::LightCyan,
    Color::LightMagenta,
];

/// Draw spectrum analyzer with per-channel frequency bars
pub fn draw_spectrum(f: &mut Frame, area: Rect, app: &App) {
    // Get per-channel spectrum data and effect status for all active channels
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
    drop(capture);

    if spectrums.is_empty() {
        return;
    }

    let bin_count = spectrums[0].len();

    // Create all bars in sequence: [ch0_bin0, ch1_bin0, ch2_bin0, ch0_bin1, ...]
    // This creates visual grouping by frequency bin
    let mut bars: Vec<Bar> = Vec::with_capacity(bin_count * channel_count);

    for bin in 0..bin_count {
        for (ch, spectrum) in spectrums.iter().enumerate() {
            let is_drum = drum_active.get(ch).copied().unwrap_or(false);
            let is_sid = sid_active.get(ch).copied().unwrap_or(false);

            // For drums: show broadband noise across mid-high frequencies
            // For SID: use the normal spectrum but with special color
            let value = if is_drum {
                // Drums show energy across mid-high bins (bins 6-14)
                // with decreasing intensity towards extremes
                let drum_intensity = if (6..=14).contains(&bin) {
                    let center_dist = ((bin as f32 - 10.0).abs() / 4.0).min(1.0);
                    80.0 * (1.0 - center_dist * 0.5)
                } else if bin > 14 {
                    30.0 // Some high frequency content
                } else {
                    10.0 // Less low frequency
                };
                drum_intensity as u64
            } else {
                (spectrum[bin] * 100.0) as u64
            };

            // Color selection: White for drums, Cyan for SID, normal otherwise
            let color = if is_drum {
                Color::White
            } else if is_sid {
                Color::Cyan
            } else {
                CHANNEL_COLORS[ch % 12]
            };

            bars.push(
                Bar::default()
                    .value(value)
                    .style(Style::default().fg(color))
                    .text_value(String::new()),
            );
        }
    }

    let bar_group = BarGroup::default().bars(&bars);

    let chart = BarChart::default()
        .block(Block::default().borders(Borders::ALL).title(" Spectrum "))
        .data(bar_group)
        .bar_width(1)
        .bar_gap(0)
        .max(100);

    f.render_widget(chart, area);
}
