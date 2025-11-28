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
use ym2149::streaming::DEFAULT_SAMPLE_RATE;
use ym2149_arkos_replayer::{ArkosPlayer, load_aks};
use ym2149_ay_replayer::{AyPlayer, CPC_UNSUPPORTED_MSG};
use ym2149_sndh_replayer::is_sndh_data;
use ym2149_ym_replayer::{Player, load_song};

use crate::args::ChipChoice;
use crate::{ArkosPlayerWrapper, AtariAudioWrapper, AyPlayerWrapper, RealtimeChip};

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
}

/// Load an Arkos Tracker (AKS) file.
fn load_arkos_file(
    file_data: &[u8],
    file_path: &str,
    _chip_choice: ChipChoice,
    color_filter_override: Option<bool>,
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

    let color_filter = color_filter_override.unwrap_or(true);

    Ok(PlayerInfo {
        player: Box::new(ArkosPlayerWrapper::new(player)) as Box<dyn RealtimeChip>,
        total_samples,
        song_info: info_str,
        color_filter,
    })
}

/// Load an SNDH (Atari ST) file using atari-audio for accurate playback.
fn load_sndh_file(
    file_data: &[u8],
    file_path: &str,
    color_filter_override: Option<bool>,
) -> ym2149_ym_replayer::Result<PlayerInfo> {
    // Decompress if ICE! packed
    let raw_data = if is_ice_packed(file_data) {
        ice_depack(file_data).map_err(|e| format!("ICE decompression failed: {e}"))?
    } else {
        file_data.to_vec()
    };

    // Parse metadata from SNDH header
    let (title, author, player_rate) = parse_sndh_metadata(&raw_data);

    let info_str = format!(
        "File: {}\nFormat: SNDH (Atari ST) via atari-audio\nTitle: {}\nAuthor: {}\nPlayer rate: {} Hz",
        file_path,
        title.as_deref().unwrap_or("(unknown)"),
        author.as_deref().unwrap_or("(unknown)"),
        player_rate,
    );

    // Create player using atari-audio
    let player = AtariAudioWrapper::new(&raw_data, DEFAULT_SAMPLE_RATE, player_rate)
        .map_err(|e| format!("atari-audio init failed: {e}"))?;

    // Estimate duration (3 minutes if unknown)
    let total_samples = DEFAULT_SAMPLE_RATE as usize * 180;
    let color_filter = color_filter_override.unwrap_or(false);

    Ok(PlayerInfo {
        player: Box::new(player) as Box<dyn RealtimeChip>,
        total_samples,
        song_info: info_str,
        color_filter,
    })
}

// --- ICE! decompression ---

const ICE_MAGIC: u32 = 0x49434521; // "ICE!"

fn is_ice_packed(data: &[u8]) -> bool {
    if data.len() < 12 {
        return false;
    }
    get_u32_be(data, 0) == ICE_MAGIC
}

fn get_u32_be(data: &[u8], offset: usize) -> u32 {
    ((data[offset] as u32) << 24)
        | ((data[offset + 1] as u32) << 16)
        | ((data[offset + 2] as u32) << 8)
        | (data[offset + 3] as u32)
}

fn ice_depack(src: &[u8]) -> Result<Vec<u8>, String> {
    if !is_ice_packed(src) {
        return Err("No ICE! header found".to_string());
    }

    let packed_size = get_u32_be(src, 4) as usize;
    let orig_size = get_u32_be(src, 8) as usize;

    if src.len() < packed_size {
        return Err(format!(
            "Data too short: expected {} bytes, got {}",
            packed_size,
            src.len()
        ));
    }

    if orig_size == 0 || orig_size > 16 * 1024 * 1024 {
        return Err(format!("Invalid original size: {}", orig_size));
    }

    let mut dst = vec![0u8; orig_size];
    let mut state = IceState::new(src, packed_size, &mut dst);
    state.depack()?;

    Ok(dst)
}

struct IceState<'a> {
    src: &'a [u8],
    src_pos: usize,
    dst: &'a mut [u8],
    dst_pos: usize,
    cmd: u8,
    mask: u8,
}

impl<'a> IceState<'a> {
    fn new(src: &'a [u8], packed_size: usize, dst: &'a mut [u8]) -> Self {
        let dst_len = dst.len();
        Self {
            src,
            src_pos: packed_size,
            dst,
            dst_pos: dst_len,
            cmd: 0,
            mask: 0,
        }
    }

    fn depack(&mut self) -> Result<(), String> {
        self.get_bits(1)?;
        self.mask = 0x80;
        while (self.cmd & 1) == 0 {
            self.cmd >>= 1;
            self.mask >>= 1;
        }
        self.cmd >>= 1;

        loop {
            if self.get_bits(1)? != 0 {
                let len = self.get_literal_length()?;
                self.copy_literal(len)?;
                if self.dst_pos == 0 {
                    return Ok(());
                }
            }

            let (len, pos) = self.get_sld_params()?;
            self.copy_sld(len, pos)?;
            if self.dst_pos == 0 {
                return Ok(());
            }
        }
    }

    fn get_bits(&mut self, mut len: u32) -> Result<u32, String> {
        let mut result = 0u32;
        while len > 0 {
            result <<= 1;
            self.mask >>= 1;
            if self.mask == 0 {
                if self.src_pos == 0 {
                    return Err("Unexpected end of compressed data".to_string());
                }
                self.src_pos -= 1;
                self.cmd = self.src[self.src_pos];
                self.mask = 0x80;
            }
            if (self.cmd & self.mask) != 0 {
                result |= 1;
            }
            len -= 1;
        }
        Ok(result)
    }

    fn get_literal_length(&mut self) -> Result<usize, String> {
        const LEN_BITS: [u32; 6] = [1, 2, 2, 3, 8, 15];
        const MAX_LEN: [u32; 6] = [1, 3, 3, 7, 255, 32768];
        const OFFSET: [usize; 6] = [1, 2, 5, 8, 15, 270];

        let mut table_pos = 0;
        let len = loop {
            let l = self.get_bits(LEN_BITS[table_pos])?;
            if l != MAX_LEN[table_pos] {
                break l;
            }
            table_pos += 1;
            if table_pos >= 6 {
                break l;
            }
        };

        let len = len as usize + OFFSET[table_pos];
        Ok(len.min(self.dst_pos))
    }

    fn copy_literal(&mut self, len: usize) -> Result<(), String> {
        for _ in 0..len {
            if self.src_pos == 0 || self.dst_pos == 0 {
                break;
            }
            self.src_pos -= 1;
            self.dst_pos -= 1;
            self.dst[self.dst_pos] = self.src[self.src_pos];
        }
        Ok(())
    }

    fn get_sld_params(&mut self) -> Result<(usize, usize), String> {
        const EXTRA_BITS: [u32; 5] = [0, 0, 1, 2, 10];
        const OFFSET: [usize; 5] = [0, 1, 2, 4, 8];

        let mut table_pos = 0;
        while self.get_bits(1)? != 0 {
            table_pos += 1;
            if table_pos == 4 {
                break;
            }
        }
        let mut len = OFFSET[table_pos] + self.get_bits(EXTRA_BITS[table_pos])? as usize;

        let pos = if len != 0 {
            const POS_EXTRA_BITS: [u32; 3] = [8, 5, 12];
            const POS_OFFSET: [usize; 3] = [32, 0, 288];

            let mut table_pos = 0;
            while self.get_bits(1)? != 0 {
                table_pos += 1;
                if table_pos == 2 {
                    break;
                }
            }
            let mut pos =
                POS_OFFSET[table_pos] + self.get_bits(POS_EXTRA_BITS[table_pos])? as usize;
            if pos != 0 {
                pos += len;
            }
            pos
        } else if self.get_bits(1)? != 0 {
            64 + self.get_bits(9)? as usize
        } else {
            self.get_bits(6)? as usize
        };

        len += 2;
        let len = len.min(self.dst_pos);

        Ok((len, pos))
    }

    fn copy_sld(&mut self, len: usize, pos: usize) -> Result<(), String> {
        let mut q = self.dst_pos + pos + 1;

        for _ in 0..len {
            if self.dst_pos == 0 {
                break;
            }
            q -= 1;
            self.dst_pos -= 1;
            if q < self.dst.len() {
                self.dst[self.dst_pos] = self.dst[q];
            }
        }
        Ok(())
    }
}

// --- SNDH metadata parsing ---

fn parse_sndh_metadata(data: &[u8]) -> (Option<String>, Option<String>, u32) {
    let mut title = None;
    let mut author = None;
    let mut player_rate = 50u32;

    if data.len() < 16 || &data[12..16] != b"SNDH" {
        return (title, author, player_rate);
    }

    // Calculate header size from BRA instruction
    let header_size = if data[1] != 0 {
        (data[1] as usize) + 2
    } else {
        let offset = ((data[2] as usize) << 8) | (data[3] as usize);
        offset + 2
    };
    let header_end = header_size.min(data.len());

    let mut pos = 16;

    while pos + 4 <= header_end {
        let tag = &data[pos..(pos + 4).min(data.len())];
        if tag.len() < 4 {
            break;
        }

        if &tag[0..4] == b"HDNS" {
            break;
        }

        if &tag[0..4] == b"TITL" {
            pos += 4;
            let (s, new_pos) = read_nt_string(data, pos);
            title = Some(s);
            pos = new_pos;
            continue;
        }

        if &tag[0..4] == b"COMM" {
            pos += 4;
            let (s, new_pos) = read_nt_string(data, pos);
            author = Some(s);
            pos = new_pos;
            continue;
        }

        // Timer tags
        if &tag[0..2] == b"TA"
            || &tag[0..2] == b"TB"
            || &tag[0..2] == b"TC"
            || &tag[0..2] == b"TD"
        {
            pos += 2;
            let (s, new_pos) = read_nt_string(data, pos);
            if let Ok(rate) = s.parse::<u32>() {
                player_rate = rate;
            }
            pos = new_pos;
            continue;
        }

        if &tag[0..2] == b"!V" {
            pos += 2;
            let (s, new_pos) = read_nt_string(data, pos);
            if let Ok(rate) = s.parse::<u32>() {
                player_rate = rate;
            }
            pos = new_pos;
            continue;
        }

        pos += 1;
    }

    (title, author, player_rate)
}

fn read_nt_string(data: &[u8], start: usize) -> (String, usize) {
    let mut end = start;
    while end < data.len() && data[end] != 0 {
        end += 1;
    }
    let s = String::from_utf8_lossy(&data[start..end]).to_string();
    (s, end + 1)
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
        return load_arkos_file(&file_data, file_path, chip_choice, color_filter_override);
    } else if extension == "ay" {
        println!("Detected format: AY (ZXAY/EMUL)\n");
        return load_ay_file(&file_data, file_path, color_filter_override);
    } else if extension == "sndh" {
        println!("Detected format: SNDH (Atari ST)\n");
        return load_sndh_file(&file_data, file_path, color_filter_override);
    }

    // Header-based detection for SNDH data even if the extension is missing
    if is_sndh_data(&file_data) {
        println!("Detected format: SNDH (Atari ST)\n");
        return load_sndh_file(&file_data, file_path, color_filter_override);
    }

    let (mut ym_player, summary) = load_song(&file_data)?;
    println!("Detected format: {}\n", summary.format);

    match chip_choice {
        ChipChoice::Ym2149 => {
            let color_filter = color_filter_override.unwrap_or(true);
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
                color_filter,
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
                color_filter: true,
            })
        }
    }
}
