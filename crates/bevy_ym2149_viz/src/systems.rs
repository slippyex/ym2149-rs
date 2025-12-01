//! Bevy systems for updating YM2149 visualization UI elements.

use crate::components::*;
use crate::helpers::{
    format_freq_label, format_note_label, frequency_to_note, get_channel_period,
    period_to_frequency,
};
use crate::uniforms::{OscilloscopeUniform, SpectrumUniform};
use bevy::prelude::*;
use bevy::ui::ComputedNode;
use bevy_ym2149::OscilloscopeBuffer;
use bevy_ym2149::playback::{PlaybackState, Ym2149Playback, Ym2149Settings};
use std::array::from_fn;
use std::f32::consts::PI;
use ym2149::Ym2149Backend;

// Oscilloscope rendering constants
const OSC_MARGIN: f32 = 6.0;
const OSC_SMOOTH_FACTOR: f32 = 0.7;
const OSC_NEW_SAMPLE_WEIGHT: f32 = 0.3;
const COLOR_BRIGHT_SCALE: f32 = 1.4;
const COLOR_DIM_LOW: f32 = 0.35;
const COLOR_DIM_HIGH: f32 = 0.65;
const COLOR_FADE_LOW: f32 = 0.2;
const COLOR_FADE_MID: f32 = 0.45;
const COLOR_FADE_HIGH: f32 = 0.55;

/// Update song title and artist text from the current playback.
pub fn update_song_info(
    playbacks: Query<&Ym2149Playback>,
    mut song_display: Query<&mut Text, With<SongInfoDisplay>>,
) {
    if let Some(playback) = playbacks.iter().next() {
        for mut text in song_display.iter_mut() {
            let song_text = if playback.song_title.is_empty() {
                "(loading...)".to_string()
            } else {
                format!(
                    "{}\nArtist: {}",
                    playback.song_title.trim(),
                    if playback.song_author.is_empty() {
                        "(unknown)".to_string()
                    } else {
                        playback.song_author.trim().to_string()
                    }
                )
            };
            text.0 = song_text;
        }
    }
}

/// Update playback status text (state, frame, volume, buffer fill).
pub fn update_status_display(
    playbacks: Query<&Ym2149Playback>,
    mut status_display: Query<&mut Text, With<PlaybackStatusDisplay>>,
) {
    if let Some(playback) = playbacks.iter().next() {
        for mut text in status_display.iter_mut() {
            let state_str = match playback.state {
                PlaybackState::Idle => "Idle",
                PlaybackState::Playing => "[>]",
                PlaybackState::Paused => "[||]",
                PlaybackState::Finished => "[END]",
            };

            let frame_pos = playback.frame_position();
            let volume_percent = (playback.volume * 100.0) as u32;
            let buffer_fill = playback
                .audio_buffer_fill()
                .map(|fill| (fill * 100.0) as u32)
                .unwrap_or(0);

            let status_text = format!(
                "Status: {}\n\
                 Frame: {}\n\
                 Volume: {}%\n\
                 Buffer: {}%",
                state_str, frame_pos, volume_percent, buffer_fill
            );

            text.0 = status_text;
        }
    }
}

/// Update per-channel note and frequency labels from current PSG state.
#[allow(clippy::type_complexity)]
pub fn update_detailed_channel_display(
    playbacks: Query<&Ym2149Playback>,
    mut label_sets: ParamSet<(
        Query<&mut Text, With<DetailedChannelDisplay>>,
        Query<(&ChannelNoteLabel, &mut Text)>,
        Query<(&ChannelFreqLabel, &mut Text)>,
    )>,
) {
    if let Some(playback) = playbacks.iter().next()
        && let Some(player) = playback.player_handle()
    {
        let player_locked = player.read();
        let chip = player_locked.get_chip();
        let regs = chip.dump_registers();

        let period_a = get_channel_period(regs[0], regs[1]);
        let period_b = get_channel_period(regs[2], regs[3]);
        let period_c = get_channel_period(regs[4], regs[5]);

        let (freq_a, note_a) = if let Some(period) = period_a {
            let freq = period_to_frequency(period);
            let note = frequency_to_note(freq);
            (Some(freq), note)
        } else {
            (None, None)
        };

        let (freq_b, note_b) = if let Some(period) = period_b {
            let freq = period_to_frequency(period);
            let note = frequency_to_note(freq);
            (Some(freq), note)
        } else {
            (None, None)
        };

        let (freq_c, note_c) = if let Some(period) = period_c {
            let freq = period_to_frequency(period);
            let note = frequency_to_note(freq);
            (Some(freq), note)
        } else {
            (None, None)
        };

        for mut text in label_sets.p0().iter_mut() {
            text.0.clear();
        }

        let note_strings = [
            format_note_label(note_a.as_deref()),
            format_note_label(note_b.as_deref()),
            format_note_label(note_c.as_deref()),
        ];
        let freq_strings = [
            format_freq_label(freq_a),
            format_freq_label(freq_b),
            format_freq_label(freq_c),
        ];

        for (label, mut text) in label_sets.p1().iter_mut() {
            let idx = label.channel.min(2);
            text.0 = note_strings[idx].clone();
        }

        for (label, mut text) in label_sets.p2().iter_mut() {
            let idx = label.channel.min(2);
            text.0 = freq_strings[idx].clone();
        }
    }
}

/// Update song progress bar and loop status labels.
#[allow(clippy::type_complexity)]
pub fn update_song_progress(
    playbacks: Query<&Ym2149Playback>,
    settings: Res<Ym2149Settings>,
    mut progress_fill: Query<&mut Node, With<SongProgressFill>>,
    mut labels: ParamSet<(
        Query<&mut Text, With<SongProgressLabel>>,
        Query<&mut Text, With<LoopStatusLabel>>,
    )>,
) {
    let mut ratio = 0.0f32;
    let looping = settings.loop_enabled;
    if let Some(playback) = playbacks.iter().next()
        && let Some(player) = playback.player_handle()
    {
        let player_locked = player.read();
        let total_frames = player_locked.frame_count().max(1);
        let current = playback.frame_position().min(total_frames as u32) as f32;
        ratio = (current / total_frames as f32).clamp(0.0, 1.0);
    }

    let percent = (ratio * 100.0).round().clamp(0.0, 100.0);

    for mut node in progress_fill.iter_mut() {
        node.width = Val::Percent(percent);
    }

    for mut text in labels.p0().iter_mut() {
        text.0 = format!("Progress {:03.0}%", percent);
    }

    for mut text in labels.p1().iter_mut() {
        text.0 = if looping {
            "Looping: on".to_string()
        } else {
            "Looping: off".to_string()
        };
    }
}

/// Update oscilloscope waveform points, heads, spectrum bars, and channel badges.
#[allow(clippy::type_complexity)]
pub fn update_oscilloscope(
    oscilloscope_buffer: Res<OscilloscopeBuffer>,
    osc_nodes: Query<&ComputedNode, With<Oscilloscope>>,
    mut osc_uniform: ResMut<OscilloscopeUniform>,
    mut spectrum_uniform: ResMut<SpectrumUniform>,
    mut node_sets: ParamSet<(
        Query<(&OscilloscopePoint, &mut Node, &mut BackgroundColor)>,
        Query<(&OscilloscopeHead, &mut Node, &mut BackgroundColor)>,
        Query<(&SpectrumBar, &mut Node, &mut BackgroundColor)>,
        Query<(&ChannelBadge, &mut Node, &mut BackgroundColor)>,
    )>,
) {
    let samples = oscilloscope_buffer.get_samples();
    let sample_len = samples.len();
    if sample_len == 0 {
        return;
    }

    let display_points = OSCILLOSCOPE_RESOLUTION;
    let window_len = sample_len.min(display_points);
    if window_len == 0 {
        return;
    }

    let recent_samples = &samples[sample_len - window_len..];
    let window_span = window_len.saturating_sub(1).max(1) as f32;
    let sample_count = recent_samples.len();
    let sample_count_f32 = sample_count as f32;

    osc_uniform.0.clear();
    osc_uniform.0.extend(recent_samples.iter().copied());

    const BASE_COLORS: [Vec3; 3] = [
        Vec3::new(1.0, 0.4, 0.4),
        Vec3::new(0.35, 1.0, 0.45),
        Vec3::new(0.45, 0.65, 1.0),
    ];
    let canvas_height = OSCILLOSCOPE_HEIGHT;
    let canvas_width = osc_nodes
        .iter()
        .next()
        .map(|node| node.size().x)
        .unwrap_or(360.0);
    let half_height = canvas_height / 2.0;
    let margin = OSC_MARGIN;

    let mut channel_means = [0.0f32; 3];
    for sample in recent_samples {
        for (ch, mean) in channel_means.iter_mut().enumerate() {
            *mean += sample[ch];
        }
    }
    for mean in &mut channel_means {
        *mean /= sample_count_f32.max(1.0);
    }

    let mut centered_samples: [Vec<f32>; 3] = from_fn(|_| vec![0.0; sample_count]);
    let mut smoothed_samples: [Vec<f32>; 3] = from_fn(|_| vec![0.0; sample_count]);
    for ch in 0..3 {
        let mut prev = 0.0;
        for (idx, sample) in recent_samples.iter().enumerate() {
            let centered = sample[ch] - channel_means[ch];
            centered_samples[ch][idx] = centered;
            let value = centered.clamp(-1.0, 1.0);
            prev = if idx == 0 {
                value
            } else {
                prev * OSC_SMOOTH_FACTOR + value * OSC_NEW_SAMPLE_WEIGHT
            };
            smoothed_samples[ch][idx] = prev;
        }
    }

    let mut channel_span = [1.0f32; 3];
    let mut channel_rms = [0.0f32; 3];
    let mut channel_latest = [0.0f32; 3];
    for ch in 0..3 {
        let mut max_val: f32 = 0.0;
        let mut sum_sq: f32 = 0.0;
        for val in &smoothed_samples[ch] {
            let abs = val.abs();
            max_val = max_val.max(abs);
            sum_sq += val * val;
        }
        channel_span[ch] = max_val.max(0.0001);
        channel_rms[ch] = (sum_sq / sample_count_f32.max(1.0)).sqrt();
        channel_latest[ch] = smoothed_samples[ch].last().copied().unwrap_or_default();
    }

    let mut channel_scales = [half_height - margin; 3];
    for (ch, scale) in channel_scales.iter_mut().enumerate() {
        *scale = (half_height - margin) / channel_span[ch];
    }

    let point_span = display_points.saturating_sub(1).max(1) as f32;
    let width_limit = (canvas_width - 2.0).max(0.0);

    let mut spectrum = [[0.0f32; 16]; 3];
    for (ch, channel_spectrum) in spectrum.iter_mut().enumerate() {
        for (bin, magnitude_slot) in channel_spectrum.iter_mut().enumerate() {
            let freq = (bin + 1) as f32;
            let mut sum_sin = 0.0;
            let mut sum_cos = 0.0;
            for (n, sample) in centered_samples[ch].iter().enumerate() {
                let phase = 2.0 * PI * freq * (n as f32) / sample_count_f32.max(1.0);
                sum_cos += *sample * phase.cos();
                sum_sin += *sample * phase.sin();
            }
            let magnitude =
                (sum_cos * sum_cos + sum_sin * sum_sin).sqrt() / sample_count_f32.max(1.0);
            *magnitude_slot = magnitude;
        }
    }

    let mut high_freq_ratio = [0.0f32; 3];
    for (ch, channel_spectrum) in spectrum.iter().enumerate() {
        let total_energy: f32 = channel_spectrum.iter().sum();
        let high_energy: f32 = channel_spectrum[8..].iter().sum();
        high_freq_ratio[ch] = if total_energy > 1e-6 {
            (high_energy / total_energy).clamp(0.0, 1.0)
        } else {
            0.0
        };
    }

    let mut channel_max_mag = [1e-6f32; 3];
    for (ch, channel_spectrum) in spectrum.iter().enumerate() {
        channel_max_mag[ch] = channel_spectrum
            .iter()
            .copied()
            .fold(1e-6, |acc, val| acc.max(val));
    }

    spectrum_uniform.0.clear();
    spectrum_uniform.0.extend(spectrum.iter().copied());

    for (point, mut node, mut color) in node_sets.p0().iter_mut() {
        let channel_index = point.channel.min(2);
        let base = BASE_COLORS[channel_index];
        let point_index = point.index.min(display_points - 1);
        let ratio = if display_points > 1 {
            point_index as f32 / point_span
        } else {
            0.0
        };
        let sample_idx = if sample_count > 1 {
            ((ratio * window_span).round() as usize).min(sample_count - 1)
        } else {
            0
        };
        let smoothed = smoothed_samples[channel_index][sample_idx];
        let x_pos = ratio * width_limit;
        let y_pos = half_height - smoothed * channel_scales[channel_index];
        node.left = Val::Px(x_pos);
        node.top = Val::Px(y_pos.clamp(0.0, canvas_height));

        let age = 1.0 - ratio;
        let intensity = smoothed.abs().clamp(0.0, 1.0);
        let fade = (age.powf(COLOR_BRIGHT_SCALE) * (COLOR_DIM_LOW + intensity * COLOR_DIM_HIGH))
            .clamp(0.0, 1.0);
        let brightness = COLOR_FADE_MID + (COLOR_FADE_HIGH * intensity);
        let color_vec = base * brightness + Vec3::splat(intensity * COLOR_FADE_LOW);

        *color = BackgroundColor(Color::srgba(
            color_vec.x.clamp(0.0, 1.0),
            color_vec.y.clamp(0.0, 1.0),
            color_vec.z.clamp(0.0, 1.0),
            fade,
        ));
    }

    for (head, mut node, mut color) in node_sets.p1().iter_mut() {
        let ch = head.channel.min(2);
        let base = BASE_COLORS[ch];
        let latest = channel_latest[ch];
        let x_pos = if display_points > 1 { width_limit } else { 0.0 };
        let y_pos = half_height - latest * channel_scales[ch];
        node.left = Val::Px(x_pos);
        node.top = Val::Px(y_pos.clamp(0.0, canvas_height));

        let glow = (latest.abs().clamp(0.0, 1.0) * OSC_SMOOTH_FACTOR) + OSC_NEW_SAMPLE_WEIGHT;
        *color = BackgroundColor(Color::srgba(
            (base.x * (0.6 + glow * 0.4)).clamp(0.0, 1.0),
            (base.y * (0.6 + glow * 0.4)).clamp(0.0, 1.0),
            (base.z * (0.6 + glow * 0.4)).clamp(0.0, 1.0),
            (0.5 + glow * 0.5).clamp(0.0, 1.0),
        ));
    }

    for (bar, mut node, mut color) in node_sets.p2().iter_mut() {
        let ch = bar.channel.min(2);
        let base = BASE_COLORS[ch];
        let magnitude = spectrum[ch][bar.bin];
        let norm = if channel_max_mag[ch] > 1e-6 {
            (magnitude / channel_max_mag[ch]).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let bar_height = (norm.powf(0.75) * 48.0).max(4.0);
        node.height = Val::Px(bar_height);

        let tint =
            base * (COLOR_DIM_LOW + norm * COLOR_DIM_HIGH) + Vec3::splat(norm * COLOR_FADE_LOW);
        *color = BackgroundColor(Color::srgba(
            tint.x.clamp(0.0, 1.0),
            tint.y.clamp(0.0, 1.0),
            tint.z.clamp(0.0, 1.0),
            (0.35 + norm * 0.55).clamp(0.0, 1.0),
        ));
    }

    for (badge, mut node, mut color) in node_sets.p3().iter_mut() {
        let ch = badge.channel.min(2);
        match badge.kind {
            BadgeKind::Amplitude => {
                let ratio = (channel_rms[ch] / channel_span[ch]).clamp(0.0, 1.0);
                node.width = Val::Px(36.0 * ratio.max(0.05));
                let base = BASE_COLORS[ch];
                let brightness = 0.4 + ratio * 0.6;
                *color = BackgroundColor(Color::srgba(
                    (base.x * brightness).clamp(0.0, 1.0),
                    (base.y * brightness).clamp(0.0, 1.0),
                    (base.z * brightness).clamp(0.0, 1.0),
                    0.85,
                ));
            }
            BadgeKind::HighFreq => {
                let ratio = high_freq_ratio[ch];
                let glow = (0.4 + ratio * 0.6).clamp(0.4, 1.0);
                let hue = Vec3::new(1.0, 0.9, 0.4);
                let base = BASE_COLORS[ch];
                let mixed = base * (1.0 - ratio) + hue * ratio;
                *color = BackgroundColor(Color::srgba(
                    (mixed.x * glow).clamp(0.0, 1.0),
                    (mixed.y * glow).clamp(0.0, 1.0),
                    (mixed.z * glow).clamp(0.0, 1.0),
                    (0.4 + ratio * 0.5).clamp(0.0, 1.0),
                ));
            }
        }
    }
}
