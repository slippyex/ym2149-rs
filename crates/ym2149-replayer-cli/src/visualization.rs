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
use ym2149_common::{channel_period, period_to_frequency};
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
    // Check if player has subsongs and get PSG count
    let (has_subsongs, psg_count, channel_count) = {
        let guard = context.player.lock();
        (
            guard.has_subsongs(),
            guard.psg_count(),
            guard.channel_count(),
        )
    };

    // Build mute keys help based on channel count
    let mute_keys = if channel_count <= 3 {
        "[1/2/3]=mute A/B/C".to_string()
    } else if channel_count <= 6 {
        "[1-6]=mute channels".to_string()
    } else {
        "[1-9,0]=mute ch 1-10".to_string()
    };

    if has_subsongs {
        println!(
            "Playback running — keys: {}, [+/-]=subsong, [space]=pause/resume, [q]=quit\n",
            mute_keys
        );
    } else {
        println!(
            "Playback running — keys: {}, [space]=pause/resume, [q]=quit\n",
            mute_keys
        );
    }
    let playback_start = Instant::now();

    // Hide cursor and add blank lines for visualization
    // 1 line for status + 3 lines per PSG (volume bars, status, highlight)
    // + separator lines between PSGs (psg_count - 1)
    let separator_lines = psg_count.saturating_sub(1);
    let viz_lines = 1 + psg_count * 3 + separator_lines;
    print!("\x1B[?25l");
    for _ in 0..viz_lines {
        println!();
    }

    // Spawn keyboard input thread
    let (tx, rx) = std::sync::mpsc::channel::<u8>();
    let input_running = Arc::new(AtomicBool::new(true));
    let input_running_clone = Arc::clone(&input_running);

    std::thread::spawn(move || {
        run_input_thread(tx, input_running_clone);
    });

    // Escape sequence state for arrow key handling
    let mut escape_state = EscapeState::new();

    // Main visualization loop
    loop {
        std::thread::sleep(std::time::Duration::from_millis(VISUALIZATION_UPDATE_MS));

        // Process keyboard input
        while let Ok(byte) = rx.try_recv() {
            if let Some(event) = escape_state.process(byte) {
                handle_key_press(event, &context.player, &context.running);
            }
        }

        // Get current state
        let stats = context.streamer.get_stats();
        let elapsed = playback_start.elapsed().as_secs_f32();
        let (snapshot, subsong_info) = {
            let guard = context.player.lock();
            let ss_info = if guard.has_subsongs() {
                Some((guard.current_subsong(), guard.subsong_count()))
            } else {
                None
            };
            (guard.visual_snapshot(), ss_info)
        };

        // Display visualization
        display_frame(
            &snapshot,
            &context.player,
            &stats,
            elapsed,
            context.streamer.fill_percentage(),
            subsong_info,
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

/// Pending escape sequence state for arrow key handling.
struct EscapeState {
    /// Buffer for escape sequence bytes
    buffer: Vec<u8>,
    /// Whether we're in an escape sequence
    in_escape: bool,
}

impl EscapeState {
    fn new() -> Self {
        Self {
            buffer: Vec::with_capacity(4),
            in_escape: false,
        }
    }

    /// Process a byte and return a completed key event if any.
    /// Returns Some(key) for regular keys, Some(special) for arrow keys.
    fn process(&mut self, byte: u8) -> Option<KeyEvent> {
        if self.in_escape {
            self.buffer.push(byte);
            // Check for complete arrow key sequence: ESC [ A/B/C/D
            if self.buffer.len() >= 2 {
                let result = match (self.buffer.first(), self.buffer.get(1)) {
                    (Some(b'['), Some(b'A')) => Some(KeyEvent::ArrowUp),
                    (Some(b'['), Some(b'B')) => Some(KeyEvent::ArrowDown),
                    (Some(b'['), Some(b'C')) => Some(KeyEvent::ArrowRight),
                    (Some(b'['), Some(b'D')) => Some(KeyEvent::ArrowLeft),
                    _ => None,
                };
                self.buffer.clear();
                self.in_escape = false;
                return result;
            }
            None
        } else if byte == 0x1B {
            // ESC character - start escape sequence
            self.in_escape = true;
            self.buffer.clear();
            None
        } else {
            Some(KeyEvent::Regular(byte))
        }
    }
}

/// Key events including special keys.
#[derive(Debug, Clone, Copy)]
enum KeyEvent {
    Regular(u8),
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
}

/// Handle keyboard input.
fn handle_key_press(
    event: KeyEvent,
    player: &Arc<Mutex<Box<dyn RealtimeChip>>>,
    running: &Arc<AtomicBool>,
) {
    match event {
        KeyEvent::Regular(key) => match key {
            // Channel mute: 1-9 for channels 0-8, 0 for channel 9
            b'1'..=b'9' => {
                let ch = (key - b'1') as usize;
                let mut guard = player.lock();
                if ch < guard.channel_count() {
                    let muted = guard.is_channel_muted(ch);
                    guard.set_channel_mute(ch, !muted);
                }
            }
            b'0' => {
                // Channel 10 (index 9)
                let mut guard = player.lock();
                if guard.channel_count() > 9 {
                    let muted = guard.is_channel_muted(9);
                    guard.set_channel_mute(9, !muted);
                }
            }
            b' ' => {
                let mut guard = player.lock();
                match guard.state() {
                    PlaybackState::Playing => guard.pause(),
                    PlaybackState::Paused | PlaybackState::Stopped => guard.play(),
                }
            }
            b'q' | b'Q' => {
                running.store(false, Ordering::Relaxed);
            }
            // Subsong navigation: + or = for next, - or _ for previous
            b'+' | b'=' => {
                let mut guard = player.lock();
                if guard.has_subsongs() {
                    let current = guard.current_subsong();
                    let count = guard.subsong_count();
                    let next = if current >= count { 1 } else { current + 1 };
                    guard.set_subsong(next);
                }
            }
            b'-' | b'_' => {
                let mut guard = player.lock();
                if guard.has_subsongs() {
                    let current = guard.current_subsong();
                    let count = guard.subsong_count();
                    let prev = if current <= 1 { count } else { current - 1 };
                    guard.set_subsong(prev);
                }
            }
            _ => {}
        },
        // Arrow keys also work for subsong navigation
        KeyEvent::ArrowUp => {
            let mut guard = player.lock();
            if guard.has_subsongs() {
                let current = guard.current_subsong();
                let count = guard.subsong_count();
                // Next subsong (wrap around)
                let next = if current >= count { 1 } else { current + 1 };
                guard.set_subsong(next);
            }
        }
        KeyEvent::ArrowDown => {
            let mut guard = player.lock();
            if guard.has_subsongs() {
                let current = guard.current_subsong();
                let count = guard.subsong_count();
                // Previous subsong (wrap around)
                let prev = if current <= 1 { count } else { current - 1 };
                guard.set_subsong(prev);
            }
        }
        KeyEvent::ArrowLeft | KeyEvent::ArrowRight => {
            // Reserved for seek functionality
        }
    }
}

/// Channel names for display (A, B, C for each PSG).
const CHANNEL_NAMES: [&str; 12] = ["A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L"];

/// Fixed column width for consistent alignment across all PSGs.
const COLUMN_WIDTH: usize = 26;

/// Extract channel data from PSG registers.
struct ChannelData {
    period: Option<u16>,
    tone_enabled: bool,
    noise_enabled: bool,
    amplitude: u8,
    env_enabled: bool,
    env_shape: &'static str,
}

fn extract_channel_data(regs: &[u8; 16], channel: usize) -> ChannelData {
    let mixer = regs[7];
    let env_shape = get_envelope_shape_name(regs[15]);

    match channel {
        0 => ChannelData {
            period: channel_period(regs[0], regs[1]),
            tone_enabled: (mixer & 0x01) == 0,
            noise_enabled: (mixer & 0x08) == 0,
            amplitude: regs[8] & 0x0F,
            env_enabled: (regs[8] & 0x10) != 0,
            env_shape,
        },
        1 => ChannelData {
            period: channel_period(regs[2], regs[3]),
            tone_enabled: (mixer & 0x02) == 0,
            noise_enabled: (mixer & 0x10) == 0,
            amplitude: regs[9] & 0x0F,
            env_enabled: (regs[9] & 0x10) != 0,
            env_shape,
        },
        2 => ChannelData {
            period: channel_period(regs[4], regs[5]),
            tone_enabled: (mixer & 0x04) == 0,
            noise_enabled: (mixer & 0x20) == 0,
            amplitude: regs[10] & 0x0F,
            env_enabled: (regs[10] & 0x10) != 0,
            env_shape,
        },
        _ => ChannelData {
            period: None,
            tone_enabled: false,
            noise_enabled: false,
            amplitude: 0,
            env_enabled: false,
            env_shape: "",
        },
    }
}

/// Display a single visualization frame.
fn display_frame(
    snapshot: &VisualSnapshot,
    player: &Arc<Mutex<Box<dyn RealtimeChip>>>,
    stats: &crate::audio::PlaybackStats,
    elapsed: f32,
    fill_pct: f32,
    subsong_info: Option<(usize, usize)>,
) {
    // Clone and detect effects from registers if not already set
    let mut snapshot = *snapshot;
    snapshot.detect_effects_from_registers();

    let psg_count = snapshot.psg_count;
    let sync_buzzer_active = snapshot.sync_buzzer;

    // Get mute states and position
    let (mute_states, pos_pct) = {
        let guard = player.lock();
        let channel_count = guard.channel_count();
        let mutes: Vec<bool> = (0..channel_count)
            .map(|ch| guard.is_channel_muted(ch))
            .collect();
        let pos = (guard.playback_position() * 100.0).clamp(0.0, 100.0);
        (mutes, pos)
    };

    // Move cursor up: 1 status line + 3 lines per PSG + separators between PSGs
    let separator_lines = psg_count.saturating_sub(1);
    let lines_up = 1 + psg_count * 3 + separator_lines;
    print!("\x1B[{}A", lines_up);

    // Format subsong info if available
    let subsong_str = match subsong_info {
        Some((current, total)) => format!(" | Subsong: {}/{}", current, total),
        None => String::new(),
    };

    // PSG count indicator for multi-PSG songs
    let psg_str = if psg_count > 1 {
        format!(" | PSGs: {}", psg_count)
    } else {
        String::new()
    };

    print!(
        "\x1B[2K\r[{:.1}s] Progress: {:>5.1}% | Buffer: {:.1}% | Overruns: {}{}{}\n",
        elapsed,
        pos_pct,
        fill_pct * 100.0,
        stats.overrun_count,
        subsong_str,
        psg_str,
    );

    // Separator line for multi-PSG display
    let separator = format!("{:-<w$}   {:-<w$}   {:-<w$}", "", "", "", w = COLUMN_WIDTH);

    // Display each PSG
    for psg_idx in 0..psg_count {
        // Print separator between PSGs (not before the first one)
        if psg_idx > 0 {
            print!("\x1B[2K\r{}\n", separator);
        }

        let regs = &snapshot.registers[psg_idx];
        let base_ch = psg_idx * 3;

        let bar_len = 12;
        let mut bars = Vec::with_capacity(3);
        let mut statuses = Vec::with_capacity(3);
        let mut highlights = Vec::with_capacity(3);

        for local_ch in 0..3 {
            let global_ch = base_ch + local_ch;
            let data = extract_channel_data(regs, local_ch);

            let bar = create_volume_bar(data.amplitude as f32 / 15.0, bar_len);
            let muted = mute_states.get(global_ch).copied().unwrap_or(false);
            let ch_name = CHANNEL_NAMES.get(global_ch).unwrap_or(&"?");

            // Format: "A    ████████████" or "A(M) ████████████" - fixed width
            bars.push(format!(
                "{}{} {}",
                ch_name,
                if muted { "(M)" } else { "   " },
                bar
            ));

            let status = create_channel_status(
                data.tone_enabled,
                data.noise_enabled,
                data.amplitude,
                data.env_enabled,
                data.env_shape,
                snapshot.sid_active[global_ch],
                snapshot.drum_active[global_ch],
                sync_buzzer_active,
            );
            statuses.push(status);

            let highlight = format_channel_highlight(
                data.period,
                data.env_enabled,
                snapshot.sid_active[global_ch],
                snapshot.drum_active[global_ch],
            );
            highlights.push(highlight);
        }

        // Print all three lines with fixed column width
        print!(
            "\x1B[2K\r{:<w$} | {:<w$} | {:<w$}\n",
            bars[0],
            bars[1],
            bars[2],
            w = COLUMN_WIDTH
        );
        print!(
            "\x1B[2K\r{:<w$} | {:<w$} | {:<w$}\n",
            statuses[0],
            statuses[1],
            statuses[2],
            w = COLUMN_WIDTH
        );
        print!(
            "\x1B[2K\r{:<w$} | {:<w$} | {:<w$}\n",
            highlights[0],
            highlights[1],
            highlights[2],
            w = COLUMN_WIDTH
        );
    }
    io::stdout().flush().ok();
}
