//! Ratatui-based TUI visualization for YM2149 playback.
//!
//! This module provides an enhanced terminal UI with:
//! - Oscilloscope waveform display per channel
//! - Spectrum analyzer with frequency bars
//! - Real-time playback status and controls
//! - Playlist overlay for directory playback

mod capture;
mod oscilloscope;
mod playlist_overlay;
mod spectrum;

pub use capture::CaptureBuffer;

use crate::VisualSnapshot;
use crate::playlist::Playlist;
use crate::streaming::StreamingContext;

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use parking_lot::Mutex;
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph},
};
use std::io::{self, stdout};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use ym2149_common::PlaybackState;

/// Minimum terminal size for TUI mode
pub const MIN_COLS: u16 = 80;
pub const MIN_ROWS: u16 = 24;

/// Check if terminal is large enough for TUI mode
pub fn terminal_supports_tui() -> bool {
    if let Ok((cols, rows)) = crossterm::terminal::size() {
        cols >= MIN_COLS && rows >= MIN_ROWS
    } else {
        false
    }
}

/// TUI Application state
pub struct App {
    /// Capture buffer for waveform/spectrum data
    pub capture: Arc<Mutex<CaptureBuffer>>,
    /// Current channel mute states
    pub mute_states: Vec<bool>,
    /// Song metadata
    pub title: String,
    pub author: String,
    pub format: String,
    pub frame_count: u32,
    pub comment: String,
    /// Playback info
    pub elapsed: f32,
    pub duration: f32,
    pub is_playing: bool,
    pub current_frame: u32,
    /// Subsong info
    pub subsong: Option<(usize, usize)>,
    /// PSG count
    pub psg_count: usize,
    /// Current snapshot for channel display
    pub snapshot: VisualSnapshot,
    /// Playlist for directory mode (None = single file mode)
    pub playlist: Option<Playlist>,
    /// Whether playlist overlay is visible
    pub show_playlist: bool,
    /// Whether playback has been started at least once (for auto-advance)
    pub has_started_playback: bool,
    /// Master volume (0.0 - 1.0)
    pub volume: f32,
}

impl App {
    pub fn new(capture: Arc<Mutex<CaptureBuffer>>) -> Self {
        Self {
            capture,
            mute_states: vec![false; 12],
            title: String::new(),
            author: String::new(),
            format: String::new(),
            frame_count: 0,
            comment: String::new(),
            elapsed: 0.0,
            duration: 0.0,
            current_frame: 0,
            is_playing: false,
            subsong: None,
            psg_count: 1,
            snapshot: VisualSnapshot {
                registers: [[0; 16]; 4],
                psg_count: 1,
                sync_buzzer: false,
                sid_active: [false; 12],
                drum_active: [false; 12],
            },
            playlist: None,
            show_playlist: false,
            has_started_playback: false,
            volume: 1.0,
        }
    }

    /// Increase volume by 5%
    pub fn volume_up(&mut self) {
        self.volume = (self.volume + 0.05).min(1.0);
    }

    /// Decrease volume by 5%
    pub fn volume_down(&mut self) {
        self.volume = (self.volume - 0.05).max(0.0);
    }

    /// Set playlist for directory mode
    pub fn set_playlist(&mut self, playlist: Playlist) {
        self.playlist = Some(playlist);
    }

    /// Toggle playlist overlay visibility
    pub fn toggle_playlist(&mut self) {
        if self.playlist.is_some() {
            self.show_playlist = !self.show_playlist;
        }
    }

    /// Update app state from loaded song metadata
    pub fn update_from_metadata(&mut self, meta: SongMetadata) {
        self.title = meta.title;
        self.author = meta.author;
        self.format = meta.format;
        self.duration = meta.duration_secs;
        self.subsong = None; // Reset, will be updated on next frame
        self.has_started_playback = true;
    }

    /// Check if we have a playlist
    pub fn has_playlist(&self) -> bool {
        self.playlist.is_some()
    }

    /// Update app state from streaming context
    pub fn update(&mut self, context: &StreamingContext, elapsed: f32) {
        self.elapsed = elapsed;

        let guard = context.player.lock();
        self.is_playing = guard.state() == PlaybackState::Playing;
        self.psg_count = guard.psg_count();
        self.snapshot = guard.visual_snapshot();

        // Update mute states
        let channel_count = guard.channel_count();
        self.mute_states.resize(channel_count, false);
        for (ch, muted) in self.mute_states.iter_mut().enumerate() {
            *muted = guard.is_channel_muted(ch);
        }

        // Update subsong info
        if guard.has_subsongs() {
            self.subsong = Some((guard.current_subsong(), guard.subsong_count()));
        }

        // Update spectrum and waveforms from all PSG register states
        let mut capture = self.capture.lock();
        capture.update_from_registers(
            &self.snapshot.registers,
            self.psg_count,
            &self.snapshot.sid_active,
            &self.snapshot.drum_active,
        );
    }
}

/// Metadata to display in the TUI
pub struct SongMetadata {
    pub title: String,
    pub author: String,
    pub format: String,
    pub duration_secs: f32,
}

impl Default for SongMetadata {
    fn default() -> Self {
        Self {
            title: String::new(),
            author: String::new(),
            format: String::new(),
            duration_secs: 180.0,
        }
    }
}

/// Callback type for loading a new player from a file path
pub type PlayerLoader =
    Box<dyn Fn(&std::path::Path) -> Option<(Box<dyn crate::RealtimeChip>, SongMetadata)>>;

/// Restore terminal to normal state.
///
/// This function is safe to call multiple times and handles errors gracefully.
fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen);
}

/// Run the TUI visualization loop with optional playlist
pub fn run_tui_loop_with_playlist(
    context: &StreamingContext,
    capture: Arc<Mutex<CaptureBuffer>>,
    metadata: SongMetadata,
    playlist: Option<Playlist>,
    player_loader: Option<PlayerLoader>,
) -> io::Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;

    // Register panic hook to restore terminal on panic
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        restore_terminal();
        original_hook(panic_info);
    }));

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = App::new(capture);

    // Set metadata from player info
    app.title = metadata.title;
    app.author = metadata.author;
    app.format = metadata.format;
    app.duration = metadata.duration_secs;

    // Set playlist if provided (and open overlay automatically)
    if let Some(pl) = playlist {
        app.show_playlist = true; // Start with playlist open
        app.set_playlist(pl);
        // Playback hasn't started yet - user must select a song first
        app.has_started_playback = false;
    } else {
        // Single file mode - playback starts immediately
        app.has_started_playback = true;
    }

    // Get initial player state
    {
        let guard = context.player.lock();
        app.psg_count = guard.psg_count();
    }

    let mut playback_start = Instant::now();
    let frame_duration = Duration::from_millis(33); // ~30 FPS

    loop {
        let frame_start = Instant::now();

        // Handle events
        // Note: Keeping nested ifs for clarity, collapsing breaks readability
        #[allow(clippy::collapsible_if)]
        if event::poll(Duration::from_millis(10))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    // Handle playlist overlay input first
                    if app.show_playlist {
                        match key.code {
                            KeyCode::Esc => {
                                // Clear search first, then close overlay
                                if let Some(ref mut pl) = app.playlist {
                                    if pl.is_searching() {
                                        pl.search_clear();
                                    } else {
                                        app.show_playlist = false;
                                    }
                                } else {
                                    app.show_playlist = false;
                                }
                            }
                            KeyCode::Char('p') | KeyCode::Char('P')
                                if app
                                    .playlist
                                    .as_ref()
                                    .map(|pl| !pl.is_searching())
                                    .unwrap_or(true) =>
                            {
                                app.show_playlist = false;
                            }
                            KeyCode::Backspace => {
                                if let Some(ref mut pl) = app.playlist {
                                    pl.search_backspace();
                                }
                            }
                            KeyCode::Up => {
                                if let Some(ref mut pl) = app.playlist {
                                    if pl.is_searching() {
                                        pl.search_previous();
                                    } else {
                                        pl.select_previous();
                                    }
                                }
                            }
                            KeyCode::Down => {
                                if let Some(ref mut pl) = app.playlist {
                                    if pl.is_searching() {
                                        pl.search_next();
                                    } else {
                                        pl.select_next();
                                    }
                                }
                            }
                            KeyCode::PageUp => {
                                if let Some(ref mut pl) = app.playlist {
                                    pl.page_up();
                                }
                            }
                            KeyCode::PageDown => {
                                if let Some(ref mut pl) = app.playlist {
                                    pl.page_down();
                                }
                            }
                            KeyCode::Enter => {
                                // Select song and switch player
                                if let Some(ref mut pl) = app.playlist {
                                    pl.search_clear();
                                }
                                if let Some(ref pl) = app.playlist {
                                    if let Some(path) = pl.selected_path() {
                                        if let Some(ref loader) = player_loader {
                                            if let Some((new_player, new_meta)) = loader(path) {
                                                context.replace_player(new_player);
                                                app.update_from_metadata(new_meta);
                                                playback_start = Instant::now();
                                                app.show_playlist = false;
                                            }
                                        }
                                    }
                                }
                            }
                            KeyCode::Char('q') | KeyCode::Char('Q')
                                if app
                                    .playlist
                                    .as_ref()
                                    .map(|pl| !pl.is_searching())
                                    .unwrap_or(true) =>
                            {
                                context.running.store(false, Ordering::Relaxed);
                                break;
                            }
                            // Type-ahead search: any other character
                            KeyCode::Char(c) => {
                                if let Some(ref mut pl) = app.playlist {
                                    pl.search_append(c);
                                }
                            }
                            _ => {}
                        }
                    } else {
                        // Normal mode input
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Char('Q') => {
                                context.running.store(false, Ordering::Relaxed);
                                break;
                            }
                            KeyCode::Char('p') | KeyCode::Char('P') => {
                                app.toggle_playlist();
                            }
                            KeyCode::Char(' ') => {
                                let mut guard = context.player.lock();
                                match guard.state() {
                                    PlaybackState::Playing => guard.pause(),
                                    _ => guard.play(),
                                }
                            }
                            KeyCode::Char(c @ '1'..='9') => {
                                let ch = (c as u8 - b'1') as usize;
                                let mut guard = context.player.lock();
                                if ch < guard.channel_count() {
                                    let muted = guard.is_channel_muted(ch);
                                    guard.set_channel_mute(ch, !muted);
                                }
                            }
                            KeyCode::Char('0') => {
                                let mut guard = context.player.lock();
                                if guard.channel_count() > 9 {
                                    let muted = guard.is_channel_muted(9);
                                    guard.set_channel_mute(9, !muted);
                                }
                            }
                            // Subsong navigation: + or Up = next, - or Down = previous
                            KeyCode::Up | KeyCode::Char('+') | KeyCode::Char('=') => {
                                let mut guard = context.player.lock();
                                if guard.has_subsongs() {
                                    let current = guard.current_subsong();
                                    let count = guard.subsong_count();
                                    let next = if current >= count { 1 } else { current + 1 };
                                    guard.set_subsong(next);
                                }
                            }
                            KeyCode::Down | KeyCode::Char('-') | KeyCode::Char('_') => {
                                let mut guard = context.player.lock();
                                if guard.has_subsongs() {
                                    let current = guard.current_subsong();
                                    let count = guard.subsong_count();
                                    let prev = if current <= 1 { count } else { current - 1 };
                                    guard.set_subsong(prev);
                                }
                            }
                            KeyCode::Right => {
                                app.volume_up();
                                context.set_volume(app.volume);
                            }
                            KeyCode::Left => {
                                app.volume_down();
                                context.set_volume(app.volume);
                            }
                            // Next/Previous song in playlist
                            KeyCode::Char(']') | KeyCode::Char('>') | KeyCode::Char('.') => {
                                if let Some(ref mut pl) = app.playlist {
                                    pl.select_next();
                                    if let Some(path) = pl.selected_path() {
                                        if let Some(ref loader) = player_loader {
                                            if let Some((new_player, new_meta)) = loader(path) {
                                                context.replace_player(new_player);
                                                app.update_from_metadata(new_meta);
                                                playback_start = Instant::now();
                                            }
                                        }
                                    }
                                }
                            }
                            KeyCode::Char('[') | KeyCode::Char('<') | KeyCode::Char(',') => {
                                if let Some(ref mut pl) = app.playlist {
                                    pl.select_previous();
                                    if let Some(path) = pl.selected_path() {
                                        if let Some(ref loader) = player_loader {
                                            if let Some((new_player, new_meta)) = loader(path) {
                                                context.replace_player(new_player);
                                                app.update_from_metadata(new_meta);
                                                playback_start = Instant::now();
                                            }
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        // Check if we should exit
        if !context.running.load(Ordering::Relaxed) {
            break;
        }

        // Update app state
        app.update(context, playback_start.elapsed().as_secs_f32());

        // Auto-advance to next song when current song ends (playlist mode only)
        // Only auto-advance if user has already selected and played a song
        if app.has_playlist() && !app.show_playlist && app.has_started_playback {
            let is_stopped = {
                let guard = context.player.lock();
                guard.state() == PlaybackState::Stopped
            };

            if is_stopped
                && let Some(ref mut pl) = app.playlist
                && let Some(path) = pl.selected_path()
                && let Some(ref loader) = player_loader
                && let Some((new_player, new_meta)) = loader(path)
            {
                pl.select_next();
                context.replace_player(new_player);
                app.update_from_metadata(new_meta);
                playback_start = Instant::now();
            }
        }

        // Draw UI
        terminal.draw(|f| draw_ui(f, &app))?;

        // Frame rate limiting
        let frame_time = frame_start.elapsed();
        if frame_time < frame_duration {
            std::thread::sleep(frame_duration - frame_time);
        }
    }

    // Restore terminal and remove panic hook
    let _ = std::panic::take_hook(); // Remove our panic hook, restore default
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    Ok(())
}

/// Draw the main UI
fn draw_ui(f: &mut Frame, app: &App) {
    let area = f.area();

    // Main layout: header, content, footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(10),   // Content
            Constraint::Length(3), // Footer
        ])
        .split(area);

    draw_header(f, chunks[0], app);
    draw_content(f, chunks[1], app);
    draw_footer(f, chunks[2], app);

    // Draw playlist overlay on top if visible
    if app.show_playlist
        && let Some(ref playlist) = app.playlist
    {
        playlist_overlay::draw_playlist_overlay(f, playlist);
    }
}

/// Draw header with title, progress, and status
fn draw_header(f: &mut Frame, area: Rect, app: &App) {
    let title = if app.title.is_empty() {
        "YM2149 Player".to_string()
    } else {
        format!("{} by {}", app.title, app.author)
    };

    let status = if app.is_playing {
        "▶ Playing"
    } else {
        "⏸ Paused"
    };

    let elapsed_str = format_time(app.elapsed);
    let duration_str = format_time(app.duration);

    let header_text = vec![Line::from(vec![
        Span::styled(" ", Style::default()),
        Span::styled(&title, Style::default().fg(Color::Cyan).bold()),
        Span::raw("  "),
        Span::styled(
            format!("{} / {}", elapsed_str, duration_str),
            Style::default().fg(Color::Yellow),
        ),
        Span::raw("  "),
        Span::styled(status, Style::default().fg(Color::Green)),
    ])];

    let header = Paragraph::new(header_text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" YM2149 Player "),
    );

    f.render_widget(header, area);
}

/// Draw main content with oscilloscope, spectrum, channels, and song info
fn draw_content(f: &mut Frame, area: Rect, app: &App) {
    // Split vertically: oscilloscope + spectrum on top, channels + info on bottom
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(55), // Oscilloscope + Spectrum
            Constraint::Percentage(45), // Channels + Song Info
        ])
        .split(area);

    // Split top section: oscilloscope left, spectrum right
    let top_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(60), // Oscilloscope
            Constraint::Percentage(40), // Spectrum
        ])
        .split(chunks[0]);

    // Draw oscilloscope
    oscilloscope::draw_oscilloscope(f, top_chunks[0], app);

    // Draw spectrum
    spectrum::draw_spectrum(f, top_chunks[1], app);

    // Split bottom section: channels left, song info right
    let bottom_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50), // Channels
            Constraint::Percentage(50), // Song Info
        ])
        .split(chunks[1]);

    // Draw channel info
    draw_channels(f, bottom_chunks[0], app);

    // Draw song info
    draw_song_info(f, bottom_chunks[1], app);
}

/// Draw channel volume bars and info
fn draw_channels(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default().borders(Borders::ALL).title(" Channels ");

    let inner = block.inner(area);
    f.render_widget(block, area);

    let channel_count = app.psg_count * 3;
    let channel_height = if channel_count > 0 {
        (inner.height as usize / channel_count).max(1)
    } else {
        1
    };

    let channel_names = ["A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L"];
    let colors = [Color::Red, Color::Green, Color::Blue];

    for psg_idx in 0..app.psg_count {
        let regs = &app.snapshot.registers[psg_idx];

        for local_ch in 0..3 {
            let global_ch = psg_idx * 3 + local_ch;
            if global_ch >= channel_count {
                break;
            }

            let y = inner.y + (global_ch * channel_height) as u16;
            if y >= inner.y + inner.height {
                break;
            }

            let amplitude = (regs[8 + local_ch] & 0x0F) as f64 / 15.0;
            let muted = app.mute_states.get(global_ch).copied().unwrap_or(false);

            let label = format!(
                " {}{} ",
                channel_names.get(global_ch).unwrap_or(&"?"),
                if muted { "(M)" } else { "   " }
            );

            let gauge = Gauge::default()
                .block(Block::default())
                .gauge_style(
                    Style::default()
                        .fg(if muted {
                            Color::DarkGray
                        } else {
                            colors[local_ch % 3]
                        })
                        .bg(Color::Black),
                )
                .ratio(amplitude)
                .label(label);

            let gauge_area = Rect {
                x: inner.x,
                y,
                width: inner.width,
                height: channel_height as u16,
            };

            f.render_widget(gauge, gauge_area);
        }
    }
}

/// Draw song information panel
fn draw_song_info(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default().borders(Borders::ALL).title(" Song Info ");

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Build info lines
    let mut lines = Vec::new();

    // Title
    if !app.title.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("Title:  ", Style::default().fg(Color::DarkGray)),
            Span::styled(&app.title, Style::default().fg(Color::White).bold()),
        ]));
    }

    // Author
    if !app.author.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("Author: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&app.author, Style::default().fg(Color::Cyan)),
        ]));
    }

    // Format
    if !app.format.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("Format: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&app.format, Style::default().fg(Color::Yellow)),
        ]));
    }

    // Frame info
    if app.frame_count > 0 {
        let frame_info = format!("{} / {}", app.current_frame, app.frame_count);
        lines.push(Line::from(vec![
            Span::styled("Frames: ", Style::default().fg(Color::DarkGray)),
            Span::styled(frame_info, Style::default().fg(Color::Green)),
        ]));
    }

    // PSG count
    if app.psg_count > 1 {
        lines.push(Line::from(vec![
            Span::styled("PSGs:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{} chips", app.psg_count),
                Style::default().fg(Color::Magenta),
            ),
        ]));
    }

    // Comment (if available)
    if !app.comment.is_empty() {
        lines.push(Line::from(Span::raw(""))); // Empty line
        // Truncate comment to fit
        let max_len = inner.width.saturating_sub(2) as usize;
        let comment = if app.comment.len() > max_len {
            format!("{}...", &app.comment[..max_len.saturating_sub(3)])
        } else {
            app.comment.clone()
        };
        lines.push(Line::from(vec![Span::styled(
            comment,
            Style::default().fg(Color::DarkGray).italic(),
        )]));
    }

    let paragraph = Paragraph::new(lines);
    f.render_widget(paragraph, inner);
}

/// Draw footer with controls help
fn draw_footer(f: &mut Frame, area: Rect, app: &App) {
    // Build controls string based on available features
    let mut controls = String::from("[1-9] Mute  [Space] Pause  [←→] Vol");

    if app.has_playlist() {
        controls.push_str("  [,/.] Prev/Next  [p] Playlist");
    }

    if app.subsong.is_some() {
        controls.push_str("  [+/-] Subsong");
    }

    controls.push_str("  [q] Quit");

    let volume_info = format!("  Vol: {}%", (app.volume * 100.0) as u32);

    let subsong_info = app
        .subsong
        .map(|(cur, total)| format!("  Subsong: {}/{}", cur, total))
        .unwrap_or_default();

    let playlist_info = app
        .playlist
        .as_ref()
        .map(|pl| format!("  [{} songs]", pl.len()))
        .unwrap_or_default();

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(controls, Style::default().fg(Color::DarkGray)),
        Span::styled(volume_info, Style::default().fg(Color::Green)),
        Span::styled(subsong_info, Style::default().fg(Color::Yellow)),
        Span::styled(playlist_info, Style::default().fg(Color::Cyan)),
    ]))
    .block(Block::default().borders(Borders::ALL));

    f.render_widget(footer, area);
}

/// Format seconds as MM:SS
fn format_time(seconds: f32) -> String {
    // Guard against NaN, infinity, or negative values
    if !seconds.is_finite() || seconds < 0.0 {
        return "--:--".to_string();
    }
    // Clamp to reasonable maximum (99:59) to prevent overflow
    let clamped = seconds.min(5999.0);
    let mins = (clamped / 60.0) as u32;
    let secs = (clamped % 60.0) as u32;
    format!("{:02}:{:02}", mins, secs)
}
