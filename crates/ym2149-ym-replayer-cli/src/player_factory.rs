//! Player instantiation and file loading.
//!
//! This module handles:
//! - Loading YM files from disk
//! - Creating appropriate player instances
//! - Setting up demo mode when no file is provided
//! - Configuring chip-specific settings

use std::env;
use std::fs;
use std::path::Path;
use ym2149::TinyYm2149;
use ym2149::streaming::DEFAULT_SAMPLE_RATE;
use ym2149_arkos_replayer::{ArkosPlayer, load_aks};
use ym2149_ay_replayer::AyPlayer;
use ym2149_ym_replayer::player::ym_player::Ym6PlayerGeneric;
use ym2149_ym_replayer::{Player, load_song};

use crate::args::ChipChoice;
use crate::{ArkosPlayerWrapper, AyPlayerWrapper, RealtimeChip};

/// Information about a loaded player.
pub struct PlayerInfo {
    /// Boxed player instance
    pub player: Box<dyn RealtimeChip>,
    /// Total samples in the song
    pub total_samples: usize,
    /// Human-readable song information
    pub song_info: String,
}

/// Load an Arkos Tracker (AKS) file.
fn load_arkos_file(
    file_data: &[u8],
    file_path: &str,
    _chip_choice: ChipChoice,
) -> ym2149_ym_replayer::Result<PlayerInfo> {
    let song = load_aks(file_data).map_err(|e| format!("Failed to load AKS file: {}", e))?;

    if song.subsongs.is_empty() {
        return Err("AKS file does not contain any subsongs".into());
    }

    let subsong = &song.subsongs[0];
    if subsong.psgs.is_empty() {
        return Err("AKS subsong defines no PSG chips".into());
    }

    // Create player for first subsong
    let player = ArkosPlayer::new(song.clone(), 0)
        .map_err(|e| format!("Failed to create Arkos player: {}", e))?;

    // Calculate estimated duration (very rough estimate)
    // AKS replay frequency is typically 50 Hz, end_position is pattern count
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

    Ok(PlayerInfo {
        player: Box::new(ArkosPlayerWrapper::new(player)) as Box<dyn RealtimeChip>,
        total_samples,
        song_info: info_str,
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

    Ok(PlayerInfo {
        player: Box::new(AyPlayerWrapper::new(player)) as Box<dyn RealtimeChip>,
        total_samples,
        song_info: info_str,
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
    println!("Loading file: {}\n", file_path);
    let file_data =
        fs::read(file_path).map_err(|e| format!("Failed to read file '{}': {}", file_path, e))?;

    // Check file extension
    let path = Path::new(file_path);
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .unwrap_or_default();

    if extension == "aks" {
        println!("Detected format: Arkos Tracker 3 (AKS)\n");
        return load_arkos_file(&file_data, file_path, chip_choice);
    } else if extension == "ay" {
        println!("Detected format: AY (ZXAY/EMUL)\n");
        return load_ay_file(&file_data, file_path, color_filter_override);
    }

    let (mut ym_player, summary) = load_song(&file_data)?;
    println!("Detected format: {}\n", summary.format);

    match chip_choice {
        ChipChoice::Ym2149 => {
            if let Some(cf) = color_filter_override {
                ym_player.get_chip_mut().set_color_filter(cf);
            }

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
            })
        }
        ChipChoice::TinyYm2149 => {
            let mut ym_player = Ym6PlayerGeneric::<TinyYm2149>::new();
            let summary = ym_player.load_data(&file_data)?;

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
    println!("No YM file specified. Running in demo mode (5 seconds).");
    println!(
        "Usage: {} <path/to/song.ym>\n",
        env::current_exe()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            .unwrap_or_else(|| "ym2149".to_string())
    );

    match chip_choice {
        ChipChoice::Ym2149 => {
            let mut demo_player = Player::new();
            let frames = vec![[0u8; 16]; 250];
            demo_player.load_frames(frames);

            let duration_secs = demo_player.get_duration_seconds();
            let total_samples = (duration_secs * DEFAULT_SAMPLE_RATE as f32) as usize;
            let info_str = format!("Demo Mode: {:.2} seconds of silence", duration_secs);

            Ok(PlayerInfo {
                player: Box::new(demo_player) as Box<dyn RealtimeChip>,
                total_samples,
                song_info: info_str,
            })
        }
        ChipChoice::TinyYm2149 => {
            let mut demo_player = Ym6PlayerGeneric::<TinyYm2149>::new();
            let frames = vec![[0u8; 16]; 250];
            demo_player.load_frames(frames);

            let duration_secs = demo_player.get_duration_seconds();
            let total_samples = (duration_secs * DEFAULT_SAMPLE_RATE as f32) as usize;
            let info_str = format!(
                "Demo Mode: {:.2} seconds of silence (tiny backend)",
                duration_secs
            );

            Ok(PlayerInfo {
                player: Box::new(demo_player) as Box<dyn RealtimeChip>,
                total_samples,
                song_info: info_str,
            })
        }
    }
}
