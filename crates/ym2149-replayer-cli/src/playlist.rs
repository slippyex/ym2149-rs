//! Playlist management for directory-based playback.
//!
//! This module provides:
//! - Recursive directory scanning for music files
//! - Metadata extraction for playlist display
//! - Song selection and loading

use std::fs;
use std::path::{Path, PathBuf};

use ym2149_arkos_replayer::load_aks;
use ym2149_ay_replayer::AyPlayer;
use ym2149_sndh_replayer::{SndhPlayer, is_sndh_data};
use ym2149_ym_replayer::load_song;

/// Supported file extensions
const SUPPORTED_EXTENSIONS: &[&str] = &["ym", "aks", "ay", "sndh"];

/// Entry in the playlist with metadata
#[derive(Clone, Debug)]
pub struct PlaylistEntry {
    /// Full path to the file
    pub path: PathBuf,
    /// Song title (from metadata or filename)
    pub title: String,
    /// Song author (from metadata)
    pub author: String,
    /// Duration in seconds (if known)
    pub duration_secs: Option<f32>,
    /// File format (YM, AKS, AY, SNDH)
    pub format: String,
}

impl PlaylistEntry {
    /// Get display string for the playlist UI
    pub fn display_string(&self) -> String {
        let duration_str = self
            .duration_secs
            .filter(|d| d.is_finite() && *d >= 0.0)
            .map(|d| {
                let clamped = d.min(5999.0); // Cap at 99:59
                let mins = (clamped / 60.0) as u32;
                let secs = (clamped % 60.0) as u32;
                format!(" ({:02}:{:02})", mins, secs)
            })
            .unwrap_or_default();

        if self.title.is_empty() || self.title == "(unknown)" {
            // Fall back to filename
            let filename = self
                .path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("???");
            format!("{}{}", filename, duration_str)
        } else if self.author.is_empty() || self.author == "(unknown)" {
            format!("{}{}", self.title, duration_str)
        } else {
            format!("{} - {}{}", self.author, self.title, duration_str)
        }
    }
}

/// Playlist containing all discovered songs
#[derive(Default)]
pub struct Playlist {
    /// All playlist entries
    pub entries: Vec<PlaylistEntry>,
    /// Currently selected index
    pub selected: usize,
    /// Current search query for type-ahead
    pub search_query: String,
}

impl Playlist {
    /// Scan a directory recursively for music files
    pub fn scan_directory(path: &Path) -> std::io::Result<Self> {
        let mut entries = Vec::new();
        scan_directory_recursive(path, &mut entries)?;

        // Sort by display string for consistent ordering
        entries.sort_by_key(|e| e.display_string().to_lowercase());

        Ok(Self {
            entries,
            selected: 0,
            search_query: String::new(),
        })
    }

    /// Check if playlist is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get number of entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Move selection up
    pub fn select_previous(&mut self) {
        if !self.entries.is_empty() {
            if self.selected == 0 {
                self.selected = self.entries.len() - 1;
            } else {
                self.selected -= 1;
            }
        }
    }

    /// Move selection down
    pub fn select_next(&mut self) {
        if !self.entries.is_empty() {
            self.selected = (self.selected + 1) % self.entries.len();
        }
    }

    /// Page up (10 items)
    pub fn page_up(&mut self) {
        if !self.entries.is_empty() {
            self.selected = self.selected.saturating_sub(10);
        }
    }

    /// Page down (10 items)
    pub fn page_down(&mut self) {
        if !self.entries.is_empty() {
            self.selected = (self.selected + 10).min(self.entries.len() - 1);
        }
    }

    /// Get currently selected entry
    pub fn selected_entry(&self) -> Option<&PlaylistEntry> {
        self.entries.get(self.selected)
    }

    /// Get path of selected entry
    pub fn selected_path(&self) -> Option<&Path> {
        self.selected_entry().map(|e| e.path.as_path())
    }

    /// Add a character to the search query and jump to first match
    pub fn search_append(&mut self, c: char) {
        self.search_query.push(c);
        self.jump_to_search_match();
    }

    /// Remove last character from search query
    pub fn search_backspace(&mut self) {
        self.search_query.pop();
        if !self.search_query.is_empty() {
            self.jump_to_search_match();
        }
    }

    /// Clear the search query
    pub fn search_clear(&mut self) {
        self.search_query.clear();
    }

    /// Check if search is active
    pub fn is_searching(&self) -> bool {
        !self.search_query.is_empty()
    }

    /// Get current search query
    pub fn search_query(&self) -> &str {
        &self.search_query
    }

    /// Jump to the first entry matching the search query.
    ///
    /// For single-character queries, prioritizes entries that START with the character
    /// (jump-to-letter behavior). For multi-character queries, matches anywhere.
    fn jump_to_search_match(&mut self) {
        if self.search_query.is_empty() || self.entries.is_empty() {
            return;
        }

        let query_lower = self.search_query.to_lowercase();
        let is_single_char = self.search_query.len() == 1;

        // For single character: prioritize "starts with" matches from the beginning
        if is_single_char {
            // Search from beginning for entries starting with the character
            for (i, entry) in self.entries.iter().enumerate() {
                if entry_starts_with(&query_lower, entry) {
                    self.selected = i;
                    return;
                }
            }
        }

        // Multi-char or no "starts with" match: search for "contains" from current position
        for (i, entry) in self.entries.iter().enumerate().skip(self.selected) {
            if entry_matches(&query_lower, entry) {
                self.selected = i;
                return;
            }
        }

        // If not found, search from the beginning
        for (i, entry) in self.entries.iter().enumerate().take(self.selected) {
            if entry_matches(&query_lower, entry) {
                self.selected = i;
                return;
            }
        }
    }

    /// Jump to next match (for repeated search)
    pub fn search_next(&mut self) {
        if self.search_query.is_empty() || self.entries.is_empty() {
            return;
        }

        let query_lower = self.search_query.to_lowercase();
        let start = (self.selected + 1) % self.entries.len();

        // Search from after current position, wrapping around
        for i in 0..self.entries.len() {
            let idx = (start + i) % self.entries.len();
            if entry_matches(&query_lower, &self.entries[idx]) {
                self.selected = idx;
                return;
            }
        }
    }

    /// Jump to previous match
    pub fn search_previous(&mut self) {
        if self.search_query.is_empty() || self.entries.is_empty() {
            return;
        }

        let query_lower = self.search_query.to_lowercase();
        let start = if self.selected == 0 {
            self.entries.len() - 1
        } else {
            self.selected - 1
        };

        // Search backwards from before current position, wrapping around
        for i in 0..self.entries.len() {
            let idx = if i <= start {
                start - i
            } else {
                self.entries.len() - (i - start)
            };
            if entry_matches(&query_lower, &self.entries[idx]) {
                self.selected = idx;
                return;
            }
        }
    }
}

/// Check if an entry matches the search query (contains)
fn entry_matches(query_lower: &str, entry: &PlaylistEntry) -> bool {
    // Match against title, author, or filename
    let title_lower = entry.title.to_lowercase();
    let author_lower = entry.author.to_lowercase();
    let filename_lower = entry
        .path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    title_lower.contains(query_lower)
        || author_lower.contains(query_lower)
        || filename_lower.contains(query_lower)
}

/// Check if an entry starts with the search query (for jump-to-letter)
fn entry_starts_with(query_lower: &str, entry: &PlaylistEntry) -> bool {
    // Check display string (which is what user sees in the list)
    let display = entry.display_string().to_lowercase();
    display.starts_with(query_lower)
}

/// Recursively scan directory for music files
fn scan_directory_recursive(path: &Path, entries: &mut Vec<PlaylistEntry>) -> std::io::Result<()> {
    if !path.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            // Recurse into subdirectory
            scan_directory_recursive(&path, entries)?;
        } else if path.is_file() {
            // Check if it's a supported file
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                let ext_lower = ext.to_ascii_lowercase();
                if SUPPORTED_EXTENSIONS.contains(&ext_lower.as_str()) {
                    // Try to extract metadata
                    if let Some(entry) = extract_metadata(&path) {
                        entries.push(entry);
                    }
                }
            }
        }
    }

    Ok(())
}

/// Extract metadata from a music file
fn extract_metadata(path: &Path) -> Option<PlaylistEntry> {
    let file_data = fs::read(path).ok()?;
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();

    let (title, author, duration_secs, format) = match extension.as_str() {
        "aks" => extract_aks_metadata(&file_data)?,
        "ay" => extract_ay_metadata(&file_data)?,
        "sndh" => extract_sndh_metadata(&file_data)?,
        _ => {
            // Try YM format (also handles header-based SNDH detection)
            if is_sndh_data(&file_data) {
                extract_sndh_metadata(&file_data)?
            } else {
                extract_ym_metadata(&file_data)?
            }
        }
    };

    Some(PlaylistEntry {
        path: path.to_path_buf(),
        title,
        author,
        duration_secs,
        format,
    })
}

/// Extract metadata from AKS file
fn extract_aks_metadata(data: &[u8]) -> Option<(String, String, Option<f32>, String)> {
    let song = load_aks(data).ok()?;

    let title = if song.metadata.title.is_empty() {
        "(unknown)".to_string()
    } else {
        song.metadata.title.clone()
    };

    let author = if song.metadata.author.is_empty() {
        "(unknown)".to_string()
    } else {
        song.metadata.author.clone()
    };

    let duration = song
        .subsongs
        .first()
        .map(|s| s.end_position as f32 / s.replay_frequency_hz);

    Some((title, author, duration, "AKS".to_string()))
}

/// Extract metadata from AY file
fn extract_ay_metadata(data: &[u8]) -> Option<(String, String, Option<f32>, String)> {
    let (_, metadata) = AyPlayer::load_from_bytes(data, 0).ok()?;

    let title = if metadata.song_name.is_empty() {
        "(unknown)".to_string()
    } else {
        metadata.song_name.clone()
    };

    let author = if metadata.author.is_empty() {
        "(unknown)".to_string()
    } else {
        metadata.author.clone()
    };

    let duration = metadata.frame_count.map(|frames| frames as f32 / 50.0);

    Some((title, author, duration, "AY".to_string()))
}

/// Extract metadata from SNDH file
fn extract_sndh_metadata(data: &[u8]) -> Option<(String, String, Option<f32>, String)> {
    use ym2149_common::ChiptunePlayer;

    let player = SndhPlayer::new(data, 44100).ok()?;
    let metadata = ChiptunePlayer::metadata(&player);

    let title = if metadata.title.is_empty() {
        "(unknown)".to_string()
    } else {
        metadata.title.to_string()
    };

    let author = if metadata.author.is_empty() {
        "(unknown)".to_string()
    } else {
        metadata.author.to_string()
    };

    // SNDH duration is often unknown
    Some((title, author, None, "SNDH".to_string()))
}

/// Extract metadata from YM file
fn extract_ym_metadata(data: &[u8]) -> Option<(String, String, Option<f32>, String)> {
    let (player, summary) = load_song(data).ok()?;

    let (title, author) = if let Some(info) = player.info() {
        (
            if info.song_name.is_empty() {
                "(unknown)".to_string()
            } else {
                info.song_name.clone()
            },
            if info.author.is_empty() {
                "(unknown)".to_string()
            } else {
                info.author.clone()
            },
        )
    } else {
        ("(unknown)".to_string(), "(unknown)".to_string())
    };

    let duration = Some(summary.total_samples() as f32 / 44100.0);
    let format = summary.format.to_string();

    Some((title, author, duration, format))
}
