//! Player instantiation and file loading.
//!
//! This module handles:
//! - Loading YM files from disk
//! - Creating appropriate player instances
//! - Setting up demo mode when no file is provided
//! - Configuring chip-specific settings

use crate::audio::DEFAULT_SAMPLE_RATE;
use std::fs;
use std::path::Path;
use ym2149::Ym2149Backend;
use ym2149_arkos_replayer::{ArkosPlayer, load_aks};
use ym2149_ay_replayer::{AyPlayer, CPC_UNSUPPORTED_MSG};
use ym2149_sndh_replayer::is_sndh_data;
use ym2149_ym_replayer::{Player, load_song};

use crate::args::ChipChoice;
use crate::{ArkosPlayerWrapper, AyPlayerWrapper, RealtimeChip, SndhPlayerWrapper};

/// Information about a loaded player.
pub struct PlayerInfo {
    /// Boxed player instance
    pub player: Box<dyn RealtimeChip>,
    /// Total samples in the song
    pub total_samples: usize,
    /// Human-readable song information
    pub song_info: String,
    /// Whether to run the ST-style post filter
    pub color_filter: bool,
    /// Song title
    pub title: String,
    /// Song author/composer
    pub author: String,
    /// File format (YM5, SNDH, AKS, etc.)
    pub format: String,
}

/// Load an Arkos Tracker (AKS) file.
fn load_arkos_file(
    file_data: &[u8],
    file_path: &str,
    _chip_choice: ChipChoice,
    color_filter_override: Option<bool>,
) -> ym2149_ym_replayer::Result<PlayerInfo> {
    let song = load_aks(file_data).map_err(|e| format!("Failed to load AKS file: {e}"))?;

    if song.subsongs.is_empty() {
        return Err("AKS file does not contain any subsongs".into());
    }

    let subsong = &song.subsongs[0];
    if subsong.psgs.is_empty() {
        return Err("AKS subsong defines no PSG chips".into());
    }

    // Extract metadata before moving song into player
    let estimated_duration = subsong.end_position as f32 / subsong.replay_frequency_hz;
    let total_samples = (estimated_duration * DEFAULT_SAMPLE_RATE as f32) as usize;

    let info_str = format!(
        "File: {}\nFormat: Arkos Tracker 3 (AKS)\n\
         Title: {}\nAuthor: {}\nComposer: {}\n\
         Subsongs: {}\nInstruments: {}\n\
         PSG Count: {} ({} channels)\n\
         Tracks: {}\nReplay Freq: {} Hz",
        file_path,
        song.metadata.title,
        song.metadata.author,
        if song.metadata.composer.is_empty() {
            "-"
        } else {
            &song.metadata.composer
        },
        song.subsongs.len(),
        song.instruments.len(),
        subsong.psgs.len(),
        subsong.psgs.len() * 3,
        subsong.tracks.len(),
        subsong.replay_frequency_hz,
    );

    // Extract title/author before creating player (song still available here)
    let title = song.metadata.title.clone();
    let author = song.metadata.author.clone();

    // Create player - song is moved, player owns Arc<AksSong>
    let player =
        ArkosPlayer::new(song, 0).map_err(|e| format!("Failed to create Arkos player: {e}"))?;

    let color_filter = color_filter_override.unwrap_or(true);

    Ok(PlayerInfo {
        player: Box::new(ArkosPlayerWrapper::new(player)) as Box<dyn RealtimeChip>,
        total_samples,
        song_info: info_str,
        color_filter,
        title,
        author,
        format: "Arkos Tracker 3 (AKS)".to_string(),
    })
}

/// Load an SNDH (Atari ST) file using ym2149-sndh-replayer for accurate playback.
fn load_sndh_file(
    file_data: &[u8],
    file_path: &str,
    color_filter_override: Option<bool>,
) -> ym2149_ym_replayer::Result<PlayerInfo> {
    // Create player using ym2149-sndh-replayer (handles ICE! decompression internally)
    let player = SndhPlayerWrapper::new(file_data, DEFAULT_SAMPLE_RATE)
        .map_err(|e| format!("SNDH player init failed: {e}"))?;

    // Get metadata from the player (which already parsed the SNDH file)
    let metadata = player.metadata();
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
    let player_rate = metadata.frame_rate;

    // Get duration from FRMS/TIME metadata (use trait method)
    use ym2149_common::ChiptunePlayerBase;
    let duration_secs = player.duration_seconds();
    let total_samples = if duration_secs > 0.0 {
        (duration_secs * DEFAULT_SAMPLE_RATE as f32) as usize
    } else {
        // Fallback: 3 minutes if duration unknown
        DEFAULT_SAMPLE_RATE as usize * 180
    };

    let duration_str = if duration_secs > 0.0 {
        let mins = (duration_secs / 60.0) as u32;
        let secs = (duration_secs % 60.0) as u32;
        format!("{mins}:{secs:02}")
    } else {
        "unknown".to_string()
    };

    let info_str = format!(
        "File: {file_path}\nFormat: SNDH (Atari ST)\nTitle: {title}\nAuthor: {author}\nPlayer rate: {player_rate} Hz\nDuration: {duration_str}"
    );

    let color_filter = color_filter_override.unwrap_or(false);

    Ok(PlayerInfo {
        player: Box::new(player) as Box<dyn RealtimeChip>,
        total_samples,
        song_info: info_str,
        color_filter,
        title,
        author,
        format: "SNDH (Atari ST)".to_string(),
    })
}

/// Load an AY (ZXAY/EMUL) file.
fn load_ay_file(
    file_data: &[u8],
    file_path: &str,
    color_filter_override: Option<bool>,
) -> ym2149_ym_replayer::Result<PlayerInfo> {
    let (mut player, metadata) =
        AyPlayer::load_from_bytes(file_data, 0).map_err(|e| format!("AY load failed: {e}"))?;

    if let Some(cf) = color_filter_override {
        player.set_color_filter(cf);
    }

    if player.requires_cpc_firmware() {
        return Err(CPC_UNSUPPORTED_MSG.into());
    }

    let samples_per_frame = (DEFAULT_SAMPLE_RATE as f32 / 50.0).round() as usize;
    let total_samples = metadata
        .frame_count
        .map(|frames| frames * samples_per_frame)
        .unwrap_or(DEFAULT_SAMPLE_RATE as usize * 180);

    let info_str = format!(
        "File: {}\nFormat: AY/EMUL\nTitle: {}\nAuthor: {}\nSongs: {}/{}\nFrame length: {}\n",
        file_path,
        metadata.song_name,
        if metadata.author.is_empty() {
            "(unknown)"
        } else {
            &metadata.author
        },
        metadata.song_index + 1,
        metadata.song_count,
        metadata
            .frame_count
            .map(|f| f.to_string())
            .unwrap_or_else(|| "unknown".into()),
    );

    let color_filter = color_filter_override.unwrap_or(true);

    Ok(PlayerInfo {
        player: Box::new(AyPlayerWrapper::new(player)) as Box<dyn RealtimeChip>,
        total_samples,
        song_info: info_str,
        color_filter,
        title: metadata.song_name.clone(),
        author: metadata.author.clone(),
        format: "AY/EMUL".to_string(),
    })
}

/// Create a player instance from a file path.
///
/// Loads the YM file, detects its format, and creates an appropriate player.
///
/// # Arguments
/// * `file_path` - Path to the YM file
/// * `chip_choice` - Which chip backend to use
/// * `color_filter_override` - Optional color filter setting
///
/// # Returns
/// PlayerInfo with the configured player and metadata
pub fn create_player(
    file_path: &str,
    chip_choice: ChipChoice,
    color_filter_override: Option<bool>,
) -> ym2149_ym_replayer::Result<PlayerInfo> {
    // Note: No println! here - TUI mode handles its own display
    let file_data =
        fs::read(file_path).map_err(|e| format!("Failed to read file '{file_path}': {e}"))?;

    // Check file extension
    let path = Path::new(file_path);
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .unwrap_or_default();

    if extension == "aks" {
        return load_arkos_file(&file_data, file_path, chip_choice, color_filter_override);
    } else if extension == "ay" {
        return load_ay_file(&file_data, file_path, color_filter_override);
    } else if extension == "sndh" {
        return load_sndh_file(&file_data, file_path, color_filter_override);
    }

    // Header-based detection for SNDH data even if the extension is missing
    if is_sndh_data(&file_data) {
        return load_sndh_file(&file_data, file_path, color_filter_override);
    }

    let (mut ym_player, summary) = load_song(&file_data)?;

    match chip_choice {
        ChipChoice::Ym2149 => {
            let color_filter = color_filter_override.unwrap_or(true);
            if let Some(cf) = color_filter_override {
                ym_player.get_chip_mut().set_color_filter(cf);
            }

            // Extract metadata
            let (title, author) = if let Some(info) = ym_player.info() {
                (info.song_name.clone(), info.author.clone())
            } else {
                (String::new(), String::new())
            };

            let info_str = format!(
                "File: {} ({})\n{}",
                file_path,
                summary.format,
                ym_player.format_info()
            );

            let total_samples = summary.total_samples();

            Ok(PlayerInfo {
                player: Box::new(ym_player) as Box<dyn RealtimeChip>,
                total_samples,
                song_info: info_str,
                color_filter,
                title,
                author,
                format: summary.format.to_string(),
            })
        }
    }
}

/// Create a demo player with silence when no file is provided.
///
/// # Arguments
/// * `chip_choice` - Which chip backend to use
///
/// # Returns
/// PlayerInfo with a demo player
pub fn create_demo_player(chip_choice: ChipChoice) -> ym2149_ym_replayer::Result<PlayerInfo> {
    // Note: No println! here - TUI mode handles its own display
    match chip_choice {
        ChipChoice::Ym2149 => {
            let mut demo_player = Player::new();
            let frames = vec![[0u8; 16]; 250];
            demo_player.load_frames(frames);

            let duration_secs = demo_player.get_duration_seconds();
            let total_samples = (duration_secs * DEFAULT_SAMPLE_RATE as f32) as usize;
            let info_str = format!("Demo Mode: {duration_secs:.2} seconds of silence");

            Ok(PlayerInfo {
                player: Box::new(demo_player) as Box<dyn RealtimeChip>,
                total_samples,
                song_info: info_str,
                color_filter: true,
                title: "Demo Mode".to_string(),
                author: String::new(),
                format: "Demo".to_string(),
            })
        }
    }
}
