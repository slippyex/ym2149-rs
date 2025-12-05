//! Playlist overlay widget for song selection.
//!
//! Displays a centered popup with a scrollable list of songs,
//! showing title, author, and duration from metadata.

use crate::playlist::Playlist;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};

/// Draw the playlist overlay popup
pub fn draw_playlist_overlay(f: &mut Frame, playlist: &Playlist) {
    let area = f.area();

    // Calculate popup size (80% width, 70% height, centered)
    let popup_width = (area.width as f32 * 0.8) as u16;
    let popup_height = (area.height as f32 * 0.7) as u16;

    let popup_area = centered_rect(popup_width, popup_height, area);

    // Clear the area behind the popup
    f.render_widget(Clear, popup_area);

    // Create the popup block with search indicator in title
    let title = if playlist.is_searching() {
        format!(" Search: {} ", playlist.search_query())
    } else {
        " Playlist - Select Song ".to_string()
    };

    let border_color = if playlist.is_searching() {
        Color::Yellow
    } else {
        Color::Cyan
    };

    let block = Block::default()
        .title(title)
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(Color::Black));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    if playlist.is_empty() {
        // Show "No songs found" message
        let msg = Paragraph::new("No supported files found in directory")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        f.render_widget(msg, inner);
        return;
    }

    // Split inner area for list and footer
    let chunks = Layout::default()
        .constraints([Constraint::Min(3), Constraint::Length(2)])
        .split(inner);

    // Get search query for highlighting
    let search_query = playlist.search_query().to_lowercase();

    // Create list items
    let items: Vec<ListItem> = playlist
        .entries
        .iter()
        .enumerate()
        .map(|(idx, entry)| {
            let display = entry.display_string();
            let is_selected = idx == playlist.selected;

            // Add format indicator
            let format_color = match entry.format.as_str() {
                "AKS" => Color::Green,
                "SNDH" => Color::Yellow,
                "AY" => Color::Magenta,
                _ => Color::Blue, // YM formats
            };

            // Build line with search highlighting
            let mut spans = vec![Span::styled(
                format!("[{}] ", entry.format),
                Style::default().fg(format_color),
            )];

            // Highlight matching text if searching
            if !search_query.is_empty() {
                let display_lower = display.to_lowercase();
                if let Some(match_start) = display_lower.find(&search_query) {
                    let match_end = match_start + search_query.len();

                    // Before match
                    if match_start > 0 {
                        let style = if is_selected {
                            Style::default()
                                .fg(Color::Black)
                                .bg(Color::Cyan)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::White)
                        };
                        spans.push(Span::styled(display[..match_start].to_string(), style));
                    }

                    // Match (highlighted)
                    let match_style = if is_selected {
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::Black).bg(Color::Yellow)
                    };
                    spans.push(Span::styled(
                        display[match_start..match_end].to_string(),
                        match_style,
                    ));

                    // After match
                    if match_end < display.len() {
                        let style = if is_selected {
                            Style::default()
                                .fg(Color::Black)
                                .bg(Color::Cyan)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::White)
                        };
                        spans.push(Span::styled(display[match_end..].to_string(), style));
                    }
                } else {
                    // No match in this entry
                    let style = if is_selected {
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::DarkGray) // Dim non-matching entries
                    };
                    spans.push(Span::styled(display, style));
                }
            } else {
                // No search active
                let style = if is_selected {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                spans.push(Span::styled(display, style));
            }

            ListItem::new(Line::from(spans))
        })
        .collect();

    // Create scrollable list
    let list = List::new(items)
        .highlight_style(Style::default().bg(Color::DarkGray))
        .highlight_symbol("> ");

    // Create list state for scrolling
    let mut list_state = ListState::default();
    list_state.select(Some(playlist.selected));

    f.render_stateful_widget(list, chunks[0], &mut list_state);

    // Footer with controls - different when searching
    let footer = if playlist.is_searching() {
        Paragraph::new(Line::from(vec![
            Span::styled(
                "[↑↓] Next/Prev match  ",
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled("[Enter] Play  ", Style::default().fg(Color::Green)),
            Span::styled("[Backspace] Delete  ", Style::default().fg(Color::DarkGray)),
            Span::styled("[Esc] Clear search", Style::default().fg(Color::Yellow)),
        ]))
    } else {
        Paragraph::new(Line::from(vec![
            Span::styled("[↑↓] Navigate  ", Style::default().fg(Color::DarkGray)),
            Span::styled("[Enter] Play  ", Style::default().fg(Color::Green)),
            Span::styled("[Type] Search  ", Style::default().fg(Color::Cyan)),
            Span::styled("[p/Esc] Close", Style::default().fg(Color::Yellow)),
        ]))
    }
    .alignment(Alignment::Center);

    f.render_widget(footer, chunks[1]);
}

/// Create a centered rectangle
fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;

    Rect {
        x,
        y,
        width: width.min(area.width),
        height: height.min(area.height),
    }
}
