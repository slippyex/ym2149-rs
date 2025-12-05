//! Mono output waveform display widget.
//!
//! Displays the mixed mono output signal using Ratatui's Canvas widget.
//! Shows the final audio output combining all channels.

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

/// Minimum peak value to avoid division by zero
const MIN_PEAK: f32 = 0.001;

/// Draw mono output waveform
pub fn draw_mono_output(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Output ");

    // Get mono output samples from capture buffer
    let capture = app.capture.lock();
    let samples: Vec<f32> = capture.mono_output().iter().copied().collect();
    drop(capture);

    if samples.is_empty() {
        f.render_widget(block, area);
        return;
    }

    // Compute DC offset (mean) and center the waveform
    let mean: f32 = samples.iter().sum::<f32>() / samples.len() as f32;
    let centered: Vec<f32> = samples.iter().map(|&s| s - mean).collect();

    // Find peak for auto-scaling
    let peak = centered.iter().map(|&s| s.abs()).fold(MIN_PEAK, f32::max);

    let canvas = Canvas::default()
        .block(block)
        .x_bounds([0.0, 100.0])
        .y_bounds([-1.0, 1.0])
        .paint(|ctx| {
            // Draw center line (zero crossing)
            ctx.draw(&CanvasLine {
                x1: 0.0,
                y1: 0.0,
                x2: 100.0,
                y2: 0.0,
                color: Color::DarkGray,
            });

            // Draw waveform
            let len = centered.len();
            let step = 100.0 / len as f64;

            for i in 1..len {
                let x1 = (i - 1) as f64 * step;
                let x2 = i as f64 * step;

                // Normalize by peak for auto-scaling
                let y1 = (centered[i - 1] / peak) as f64 * 0.9;
                let y2 = (centered[i] / peak) as f64 * 0.9;

                ctx.draw(&CanvasLine {
                    x1,
                    y1,
                    x2,
                    y2,
                    color: Color::White,
                });
            }
        });

    f.render_widget(canvas, area);
}
