//! Terminal-based visualization and user interaction.
//!
//! This module provides:
//! - Real-time channel visualization
//! - Frequency and note detection
//! - Keyboard input handling
//! - Progress display

use crate::audio::VISUALIZATION_UPDATE_MS;
use crate::viz_helpers::{create_channel_status, create_volume_bar};
use parking_lot::Mutex;
use std::io::{self, Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use ym2149::util::{channel_period, period_to_frequency};
use ym2149_ym_replayer::PlaybackState;

use crate::streaming::StreamingContext;
use crate::{RealtimeChip, VisualSnapshot};

const NOTE_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

/// Get envelope shape name from register value.
fn get_envelope_shape_name(shape_val: u8) -> &'static str {
    match shape_val & 0x0F {
        0x00..=0x03 => "AD",
        0x04 => "ADR",
        0x05 => "ASR",
        0x06 => "TRI",
        0x07 => "TRISUS",
        0x08 => "SAWDN",
        0x09 => "ASAWDN",
        0x0A => "SUSSAWDN",
        0x0B => "ASSAWDN",
        0x0C => "SAWUP",
        0x0D | 0x0F => "AH",
        0x0E => "SAWDN1x",
        _ => "",
    }
}

/// Convert frequency to musical note label (e.g., "A4").
fn frequency_to_note_label(freq: f32) -> Option<String> {
    if !(freq.is_finite()) || freq <= 0.0 {
        return None;
    }
    let midi = 69.0 + 12.0 * (freq / 440.0).log2();
    let midi_rounded = midi.round();
    if !(0.0..=127.0).contains(&midi_rounded) {
        return None;
    }
    let midi_int = midi_rounded as i32;
    let note_index = ((midi_int % 12) + 12) % 12;
    let octave = (midi_int / 12) - 1;
    Some(format!("{}{}", NOTE_NAMES[note_index as usize], octave))
}

/// Format channel highlight with frequency, note, and effects.
fn format_channel_highlight(
    period: Option<u16>,
    env_enabled: bool,
    sid_enabled: bool,
    drum_enabled: bool,
) -> String {
    match period {
        Some(period) => {
            let freq = period_to_frequency(period);
            let note = frequency_to_note_label(freq).unwrap_or_default();
            let mut parts = vec![format!("{freq:>7.1}Hz")];
            if !note.is_empty() {
                parts.push(note);
            }
            if env_enabled {
                parts.push("ENV".into());
            }
            if sid_enabled {
                parts.push("SID".into());
            }
            if drum_enabled {
                parts.push("DRUM".into());
            }
            parts.join(" ")
        }
        None => {
            let mut labels: Vec<String> = Vec::new();
            if env_enabled {
                labels.push("ENV".into());
            }
            if sid_enabled {
                labels.push("SID".into());
            }
            if drum_enabled {
                labels.push("DRUM".into());
            }
            if labels.is_empty() {
                "--".to_string()
            } else {
                labels.join(" ")
            }
        }
    }
}

/// Restore terminal to normal mode (echo, canonical).
#[cfg(unix)]
fn restore_terminal_mode() {
    let _ = std::process::Command::new("stty")
        .arg("echo")
        .arg("-raw")
        .status();
}

#[cfg(not(unix))]
fn restore_terminal_mode() {}

/// Run the visualization loop with keyboard input handling.
///
/// This function:
/// - Sets up terminal raw mode for keyboard input
/// - Spawns input thread
/// - Runs visualization update loop
/// - Handles playback control keys
/// - Restores terminal on exit
pub fn run_visualization_loop(context: &StreamingContext) {
    println!("Playback running — keys: [1/2/3]=mute A/B/C, [space]=pause/resume, [q]=quit\n");
    let playback_start = Instant::now();

    // Hide cursor and add blank lines for visualization
    print!("\x1B[?25l");
    for _ in 0..4 {
        println!();
    }

    // Spawn keyboard input thread
    let (tx, rx) = std::sync::mpsc::channel::<u8>();
    let input_running = Arc::new(AtomicBool::new(true));
    let input_running_clone = Arc::clone(&input_running);

    std::thread::spawn(move || {
        run_input_thread(tx, input_running_clone);
    });

    // Main visualization loop
    loop {
        std::thread::sleep(std::time::Duration::from_millis(VISUALIZATION_UPDATE_MS));

        // Process keyboard input
        while let Ok(key) = rx.try_recv() {
            handle_key_press(key, &context.player, &context.running);
        }

        // Get current state
        let stats = context.streamer.get_stats();
        let elapsed = playback_start.elapsed().as_secs_f32();
        let snapshot = {
            let guard = context.player.lock();
            guard.visual_snapshot()
        };

        // Display visualization
        display_frame(
            &snapshot,
            &context.player,
            &stats,
            elapsed,
            context.streamer.fill_percentage(),
        );

        if !context.running.load(Ordering::Relaxed) {
            break;
        }
    }

    // Cleanup
    restore_terminal_mode();
    println!("\x1B[?25h");
    io::stdout().flush().ok();

    input_running.store(false, Ordering::Relaxed);
}

/// Run keyboard input thread in raw mode.
fn run_input_thread(tx: std::sync::mpsc::Sender<u8>, running: Arc<AtomicBool>) {
    #[cfg(unix)]
    let _ = std::process::Command::new("stty")
        .arg("-echo")
        .arg("raw")
        .status();

    let mut stdin = io::stdin();
    let mut buf = [0u8; 1];

    while running.load(Ordering::Relaxed) {
        if stdin.read_exact(&mut buf).is_ok() {
            let _ = tx.send(buf[0]);
            if buf[0] == b'\x03' {
                break;
            }
        }
    }

    #[cfg(unix)]
    let _ = std::process::Command::new("stty")
        .arg("echo")
        .arg("-raw")
        .status();
}

/// Handle keyboard input.
fn handle_key_press(
    key: u8,
    player: &Arc<Mutex<Box<dyn RealtimeChip>>>,
    running: &Arc<AtomicBool>,
) {
    match key {
        b'1' | b'2' | b'3' => {
            let ch = (key - b'1') as usize;
            let mut guard = player.lock();
            let muted = guard.is_channel_muted(ch);
            guard.set_channel_mute(ch, !muted);
        }
        b' ' => {
            let mut guard = player.lock();
            match guard.state() {
                PlaybackState::Playing => {
                    let _ = guard.pause();
                }
                PlaybackState::Paused | PlaybackState::Stopped => {
                    let _ = guard.play();
                }
            }
        }
        b'q' | b'Q' => {
            running.store(false, Ordering::Relaxed);
        }
        _ => {}
    }
}

/// Display a single visualization frame.
fn display_frame(
    snapshot: &VisualSnapshot,
    player: &Arc<Mutex<Box<dyn RealtimeChip>>>,
    stats: &crate::audio::PlaybackStats,
    elapsed: f32,
    fill_pct: f32,
) {
    let regs = snapshot.registers;
    let mixer_r7 = regs[7];
    let envelope_shape_r15 = regs[15];

    let period_a = channel_period(regs[0], regs[1]);
    let period_b = channel_period(regs[2], regs[3]);
    let period_c = channel_period(regs[4], regs[5]);

    let tone_a = (mixer_r7 & 0x01) == 0;
    let tone_b = (mixer_r7 & 0x02) == 0;
    let tone_c = (mixer_r7 & 0x04) == 0;

    let noise_a = (mixer_r7 & 0x08) == 0;
    let noise_b = (mixer_r7 & 0x10) == 0;
    let noise_c = (mixer_r7 & 0x20) == 0;

    let amp_a = regs[8] & 0x0F;
    let amp_b = regs[9] & 0x0F;
    let amp_c = regs[10] & 0x0F;

    let env_a = (regs[8] & 0x10) != 0;
    let env_b = (regs[9] & 0x10) != 0;
    let env_c = (regs[10] & 0x10) != 0;

    let env_shape = get_envelope_shape_name(envelope_shape_r15);

    let bar_len = 10;
    let bar_a = create_volume_bar(amp_a as f32 / 15.0, bar_len);
    let bar_b = create_volume_bar(amp_b as f32 / 15.0, bar_len);
    let bar_c = create_volume_bar(amp_c as f32 / 15.0, bar_len);

    let sync_buzzer_active = snapshot.sync_buzzer;
    let sid_active = snapshot.sid_active;
    let drum_active = snapshot.drum_active;

    let highlight_a = format_channel_highlight(period_a, env_a, sid_active[0], drum_active[0]);
    let highlight_b = format_channel_highlight(period_b, env_b, sid_active[1], drum_active[1]);
    let highlight_c = format_channel_highlight(period_c, env_c, sid_active[2], drum_active[2]);

    let status_a = create_channel_status(
        tone_a,
        noise_a,
        amp_a,
        env_a,
        env_shape,
        sid_active[0],
        drum_active[0],
        sync_buzzer_active,
    );
    let status_b = create_channel_status(
        tone_b,
        noise_b,
        amp_b,
        env_b,
        env_shape,
        sid_active[1],
        drum_active[1],
        sync_buzzer_active,
    );
    let status_c = create_channel_status(
        tone_c,
        noise_c,
        amp_c,
        env_c,
        env_shape,
        sid_active[2],
        drum_active[2],
        sync_buzzer_active,
    );

    let (muted_a, muted_b, muted_c) = {
        let guard = player.lock();
        (
            guard.is_channel_muted(0),
            guard.is_channel_muted(1),
            guard.is_channel_muted(2),
        )
    };

    let pos_pct = {
        let guard = player.lock();
        (guard.get_playback_position() * 100.0).clamp(0.0, 100.0)
    };

    // Move cursor up and redraw
    print!("\x1B[4A");
    print!(
        "\x1B[2K\r[{:.1}s] Progress: {:>5.1}% | Buffer: {:.1}%b | Overruns: {}\n",
        elapsed,
        pos_pct,
        fill_pct * 100.0,
        stats.overrun_count,
    );
    print!(
        "\x1B[2K\rA{} {:<18} | B{} {:<18} | C{} {:<18}\n",
        if muted_a { "(M)" } else { "  " },
        bar_a,
        if muted_b { "(M)" } else { "  " },
        bar_b,
        if muted_c { "(M)" } else { "  " },
        bar_c,
    );
    print!(
        "\x1B[2K\r{:<22} | {:<22} | {:<22}\n",
        status_a, status_b, status_c
    );
    print!(
        "\x1B[2K\r{:<22} | {:<22} | {:<22}\n",
        highlight_a, highlight_b, highlight_c
    );
    io::stdout().flush().ok();
}
