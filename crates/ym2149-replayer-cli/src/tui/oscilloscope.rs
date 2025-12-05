//! Oscilloscope waveform display widget.
//!
//! Displays per-channel waveforms using Ratatui's Canvas widget.
//! Supports up to 12 channels (4 PSGs × 3 channels) for multi-PSG configurations.
//! Uses downsampling and smoothing for fluid animation.

use super::App;
use ratatui::{
    Frame,
    prelude::*,
    style::Color,
    widgets::{
        Block, Borders,
        canvas::{Canvas, Line as CanvasLine},
    },
};

/// Channel colors - cycles through for multi-PSG
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

/// Channel labels for multi-PSG
const CHANNEL_LABELS: [&str; 12] = [
    "A", "B", "C", // PSG 0
    "D", "E", "F", // PSG 1
    "G", "H", "I", // PSG 2
    "J", "K", "L", // PSG 3
];

/// Minimum peak value to avoid division by zero
const MIN_PEAK: f32 = 0.001;

/// Number of display points for smoother rendering (less = smoother but less detail)
const DISPLAY_POINTS: usize = 64;

/// Cubic interpolation for smooth waveform
fn cubic_interpolate(y0: f32, y1: f32, y2: f32, y3: f32, t: f32) -> f32 {
    let a0 = y3 - y2 - y0 + y1;
    let a1 = y0 - y1 - a0;
    let a2 = y2 - y0;
    let a3 = y1;
    a0 * t * t * t + a1 * t * t + a2 * t + a3
}

/// Downsample and smooth waveform data
fn smooth_waveform(samples: &[f32], target_len: usize) -> Vec<f32> {
    if samples.is_empty() || target_len == 0 {
        return vec![0.0; target_len];
    }

    let src_len = samples.len();
    if src_len <= target_len {
        // Not enough samples, just pad/repeat
        let mut result = samples.to_vec();
        result.resize(target_len, *samples.last().unwrap_or(&0.0));
        return result;
    }

    let mut result = Vec::with_capacity(target_len);
    let step = (src_len - 1) as f32 / (target_len - 1).max(1) as f32;

    for i in 0..target_len {
        let pos = i as f32 * step;
        let idx = pos as usize;
        let t = pos - idx as f32;

        // Get 4 points for cubic interpolation (with boundary handling)
        let y0 = samples[idx.saturating_sub(1).min(src_len - 1)];
        let y1 = samples[idx.min(src_len - 1)];
        let y2 = samples[(idx + 1).min(src_len - 1)];
        let y3 = samples[(idx + 2).min(src_len - 1)];

        result.push(cubic_interpolate(y0, y1, y2, y3, t));
    }

    result
}

/// Draw oscilloscope with dynamic channel count
pub fn draw_oscilloscope(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Oscilloscope ");

    // Get waveform data and effect status for all active channels
    let capture = app.capture.lock();
    let channel_count = capture.channel_count();
    let waveforms: Vec<Vec<f32>> = (0..channel_count)
        .map(|ch| capture.waveform(ch).iter().copied().collect())
        .collect();
    let sid_active: Vec<bool> = (0..channel_count)
        .map(|ch| capture.is_sid_active(ch))
        .collect();
    let drum_active: Vec<bool> = (0..channel_count)
        .map(|ch| capture.is_drum_active(ch))
        .collect();
    let buzz_active: Vec<bool> = (0..channel_count)
        .map(|ch| capture.is_buzz_active(ch))
        .collect();
    drop(capture);

    // Pre-process waveforms: smooth, center, and compute peak for auto-scaling
    let processed: Vec<(Vec<f32>, f32)> = waveforms
        .iter()
        .map(|waveform| {
            if waveform.is_empty() {
                return (vec![0.0; DISPLAY_POINTS], 1.0);
            }

            // Compute DC offset (mean)
            let mean: f32 = waveform.iter().sum::<f32>() / waveform.len() as f32;

            // Center the waveform
            let centered: Vec<f32> = waveform.iter().map(|&s| s - mean).collect();

            // Smooth/downsample for fluid display
            let smoothed = smooth_waveform(&centered, DISPLAY_POINTS);

            // Find peak after smoothing
            let peak = smoothed.iter().map(|&s| s.abs()).fold(MIN_PEAK, f32::max);

            (smoothed, peak)
        })
        .collect();

    // Find global peak for consistent scaling across all channels
    let global_peak = processed
        .iter()
        .map(|(_, peak)| *peak)
        .fold(MIN_PEAK, f32::max);

    // Calculate amplitude scale based on channel count
    // More channels = smaller amplitude per channel
    let amplitude_scale = match channel_count {
        1..=3 => 0.45, // Single PSG: 45% of row height
        4..=6 => 0.35, // 2 PSGs: 35% of row height
        7..=9 => 0.28, // 3 PSGs: 28% of row height
        _ => 0.22,     // 4 PSGs: 22% of row height
    };

    let y_bounds = channel_count as f64;

    let canvas = Canvas::default()
        .block(block)
        .x_bounds([0.0, 100.0])
        .y_bounds([0.0, y_bounds])
        .paint(|ctx| {
            for (ch, (centered, _)) in processed.iter().enumerate() {
                let color = CHANNEL_COLORS[ch % 12];
                // Channels from top to bottom (reversed index)
                let y_base = (channel_count - 1 - ch) as f64 + 0.5;

                // Draw center line (zero crossing) first
                ctx.draw(&CanvasLine {
                    x1: 0.0,
                    y1: y_base,
                    x2: 100.0,
                    y2: y_base,
                    color: Color::DarkGray,
                });

                // Check for special effects
                let is_sid = sid_active.get(ch).copied().unwrap_or(false);
                let is_drum = drum_active.get(ch).copied().unwrap_or(false);
                let is_buzz = buzz_active.get(ch).copied().unwrap_or(false);

                if is_drum {
                    // DigiDrum: draw a distinctive "noise burst" pattern
                    // Use pseudo-random noise pattern to indicate sample playback
                    for x in (0..100).step_by(2) {
                        let noise_val = ((x * 31 + ch as i32 * 17) % 100) as f64 / 100.0;
                        let y1 = y_base + (noise_val - 0.5) * amplitude_scale * 1.5;
                        let y2 = y_base + (1.0 - noise_val - 0.5) * amplitude_scale * 1.5;
                        ctx.draw(&CanvasLine {
                            x1: x as f64,
                            y1,
                            x2: (x + 1) as f64,
                            y2,
                            color: Color::White, // White for drums
                        });
                    }
                } else if !centered.is_empty() {
                    // Color priority: SID (cyan) > Buzz (yellow/orange) > normal
                    let wave_color = if is_sid {
                        Color::Cyan
                    } else if is_buzz {
                        Color::Rgb(255, 180, 50) // Orange for buzz/envelope
                    } else {
                        color
                    };
                    // Normal waveform: draw as connected lines
                    let len = centered.len();
                    let step = 100.0 / len as f64;

                    for i in 1..len {
                        let x1 = (i - 1) as f64 * step;
                        let x2 = i as f64 * step;

                        // Auto-scale: normalize by global peak, then scale to fit row
                        let normalized1 = centered[i - 1] / global_peak;
                        let normalized2 = centered[i] / global_peak;

                        let y1 = y_base + normalized1 as f64 * amplitude_scale;
                        let y2 = y_base + normalized2 as f64 * amplitude_scale;

                        ctx.draw(&CanvasLine {
                            x1,
                            y1,
                            x2,
                            y2,
                            color: wave_color,
                        });
                    }
                }

                // Draw channel label with effect indicator
                let label = CHANNEL_LABELS[ch % 12];
                let label_color = if is_drum {
                    Color::White
                } else if is_sid {
                    Color::Cyan
                } else if is_buzz {
                    Color::Rgb(255, 180, 50) // Orange for buzz
                } else {
                    color
                };
                let label_text = if is_drum {
                    format!("{}♪", label)
                } else if is_sid {
                    format!("{}~", label)
                } else if is_buzz {
                    format!("{}≈", label) // ≈ symbol for buzz/envelope
                } else {
                    label.to_string()
                };
                ctx.print(
                    2.0,
                    y_base + 0.2,
                    Line::styled(label_text, Style::default().fg(label_color)),
                );
            }
        });

    f.render_widget(canvas, area);
}
