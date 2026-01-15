//! SNDH file format parser.
//!
//! SNDH is a standard format for Atari ST music that embeds the original
//! 68000 replay code along with the music data. The format uses a BRA
//! instruction at the start followed by various metadata tags.
//!
//! ## Header Format
//!
//! - Byte 0: `0x60` (BRA.s or BRA.w opcode)
//! - Byte 1: Offset (for BRA.s) or 0x00 (for BRA.w)
//! - Bytes 2-3: Offset (for BRA.w, big-endian)
//! - Bytes 12-15: "SNDH" magic
//! - Following: Tag-based metadata until "HDNS" end tag
//!
//! ## Entry Points (relative to load address)
//!
//! - +0: Initialize (D0 = subsong number, 1-based)
//! - +4: Exit/cleanup
//! - +8: Play one frame

use crate::error::{Result, SndhError};
use crate::ice::{ice_depack, is_ice_packed};

/// Maximum number of subsongs supported
const MAX_SUBSONGS: usize = 128;

/// SNDH file representation
#[derive(Debug, Clone)]
pub struct SndhFile {
    /// Raw (decompressed) SNDH data
    pub data: Vec<u8>,
    /// Parsed metadata
    pub metadata: SndhMetadata,
}

/// SNDH feature flags (from FLAG tag, SNDH v2.2)
#[derive(Debug, Clone, Default)]
pub struct SndhFlags {
    /// Uses Timer A
    pub timer_a: bool,
    /// Uses Timer B
    pub timer_b: bool,
    /// Uses Timer C
    pub timer_c: bool,
    /// Uses Timer D
    pub timer_d: bool,
    /// Requires STE hardware
    pub ste: bool,
    /// Contains sound effects
    pub sfx: bool,
    /// Uses digital/sample playback
    pub digital: bool,
    /// Uses HBL (Horizontal Blank)
    pub hbl: bool,
    /// Contains jingles
    pub jingles: bool,
    /// Kills system (takes over machine)
    pub kill_system: bool,
    /// Uses LMC1992 (STE audio mixer)
    pub lmc: bool,
    /// Uses AGA (Amiga graphics - rare)
    pub aga: bool,
    /// Uses DSP (Falcon)
    pub dsp: bool,
    /// Uses YM2149
    pub ym2149: bool,
    /// Uses Blitter
    pub blitter: bool,
    /// Requires 68020+
    pub cpu_68020: bool,
    /// Uses filters
    pub filters: bool,
    /// Stereo output
    pub stereo: bool,
    /// DMA sample rate (STE/Falcon)
    pub dma_rate: Option<DmaSampleRate>,
}

/// DMA sample rates for STE and Falcon
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmaSampleRate {
    /// 6.25 kHz (STE)
    Rate6258,
    /// 12.5 kHz (STE/Falcon)
    Rate12517,
    /// 25 kHz (STE/Falcon)
    Rate25033,
    /// 50 kHz (STE/Falcon)
    Rate50066,
    /// 12.2 kHz (Falcon only)
    Rate12292Falcon,
    /// 14 kHz (Falcon only)
    Rate14049Falcon,
    /// 16.3 kHz (Falcon only)
    Rate16390Falcon,
    /// 19.6 kHz (Falcon only)
    Rate19668Falcon,
    /// 24.5 kHz (Falcon only)
    Rate24585Falcon,
    /// 32.8 kHz (Falcon only)
    Rate32780Falcon,
    /// 49.1 kHz (Falcon only)
    Rate49170Falcon,
}

/// SNDH file metadata
#[derive(Debug, Clone, Default)]
pub struct SndhMetadata {
    /// Song title
    pub title: Option<String>,
    /// Composer/author name
    pub author: Option<String>,
    /// Year of creation
    pub year: Option<String>,
    /// Ripper name
    pub ripper: Option<String>,
    /// Converter name
    pub converter: Option<String>,
    /// Number of subsongs
    pub subsong_count: usize,
    /// Default subsong (1-based)
    pub default_subsong: usize,
    /// Player tick rate in Hz (default: 50)
    pub player_rate: u32,
    /// Duration of each subsong in seconds (from TIME tag, legacy)
    pub subsong_durations: Vec<u16>,
    /// Frame count for each subsong (from FRMS tag, SNDH v2.2)
    /// 0 = endless loop, otherwise exact frame count
    pub subsong_frames: Vec<u32>,
    /// Subtune names (from #!SN tag)
    pub subtune_names: Vec<String>,
    /// Which timer is used (A, B, C, D) - from TA/TB/TC/TD tags
    pub timer_used: Option<char>,
    /// Feature flags (from FLAG tag, SNDH v2.2)
    pub flags: SndhFlags,
}

/// Information about a specific subsong
#[derive(Debug, Clone)]
pub struct SubsongInfo {
    /// Total number of subsongs
    pub subsong_count: usize,
    /// Number of ticks/frames to play (0 = unknown/infinite)
    pub player_tick_count: u32,
    /// Player tick rate in Hz
    pub player_tick_rate: u32,
    /// Samples per tick at given sample rate
    pub samples_per_tick: u32,
    /// Song title
    pub title: Option<String>,
    /// Author name
    pub author: Option<String>,
    /// Year
    pub year: Option<String>,
    /// Subtune name (from #!SN tag, if available)
    pub subtune_name: Option<String>,
}

impl SndhFile {
    /// Parse SNDH data from raw bytes.
    ///
    /// Handles ICE! decompression automatically if needed.
    pub fn parse(data: &[u8]) -> Result<Self> {
        // Decompress if ICE! packed
        let raw_data = if is_ice_packed(data) {
            ice_depack(data)?
        } else {
            data.to_vec()
        };

        // Validate minimum size
        if raw_data.len() < 16 {
            return Err(SndhError::DataTooShort {
                expected: 16,
                actual: raw_data.len(),
            });
        }

        // Check for SNDH magic at offset 12
        if &raw_data[12..16] != b"SNDH" {
            return Err(SndhError::InvalidHeader(
                "Missing SNDH magic at offset 12".to_string(),
            ));
        }

        // Check for BRA instruction
        if raw_data[0] != 0x60 {
            return Err(SndhError::InvalidHeader(
                "Missing BRA instruction at offset 0".to_string(),
            ));
        }

        let metadata = Self::parse_metadata(&raw_data)?;

        Ok(Self {
            data: raw_data,
            metadata,
        })
    }

    /// Parse metadata tags from SNDH header.
    fn parse_metadata(data: &[u8]) -> Result<SndhMetadata> {
        let mut meta = SndhMetadata {
            subsong_count: 1,
            default_subsong: 1,
            player_rate: 50,
            ..Default::default()
        };

        // Calculate header size from BRA instruction
        let header_size = if data[1] != 0 {
            // BRA.s: 8-bit signed offset + 2
            (data[1] as usize) + 2
        } else {
            // BRA.w: 16-bit offset + 2
            let offset = ((data[2] as usize) << 8) | (data[3] as usize);
            offset + 2
        };

        let header_end = header_size.min(data.len());

        // Start parsing tags after "SNDH" magic
        let mut pos = 16;

        while pos + 4 <= header_end {
            // Skip padding/null bytes (common in SNDH for alignment)
            while pos < header_end && data[pos] == 0 {
                pos += 1;
            }
            if pos + 4 > header_end {
                break;
            }

            // Get tag (4 bytes)
            let tag = &data[pos..(pos + 4).min(data.len())];

            if tag.len() < 4 {
                break;
            }

            // Check for end marker
            if &tag[0..4] == b"HDNS" {
                break;
            }

            // 4-character tags with string values
            if &tag[0..4] == b"TITL" {
                pos += 4;
                let (s, new_pos) = Self::read_nt_string(data, pos);
                meta.title = Some(s);
                pos = new_pos;
                continue;
            }

            if &tag[0..4] == b"COMM" {
                pos += 4;
                let (s, new_pos) = Self::read_nt_string(data, pos);
                meta.author = Some(s);
                pos = new_pos;
                continue;
            }

            if &tag[0..4] == b"YEAR" {
                pos += 4;
                let (s, new_pos) = Self::read_nt_string(data, pos);
                if !s.is_empty() {
                    meta.year = Some(s);
                }
                pos = new_pos;
                continue;
            }

            if &tag[0..4] == b"RIPP" {
                pos += 4;
                let (s, new_pos) = Self::read_nt_string(data, pos);
                meta.ripper = Some(s);
                pos = new_pos;
                continue;
            }

            if &tag[0..4] == b"CONV" {
                pos += 4;
                let (s, new_pos) = Self::read_nt_string(data, pos);
                meta.converter = Some(s);
                pos = new_pos;
                continue;
            }

            if &tag[0..4] == b"TIME" {
                pos += 4;
                // Read 16-bit duration for each subsong (legacy, in seconds)
                for _ in 0..meta.subsong_count.min(MAX_SUBSONGS) {
                    if pos + 2 > data.len() {
                        break;
                    }
                    let duration = ((data[pos] as u16) << 8) | (data[pos + 1] as u16);
                    meta.subsong_durations.push(duration);
                    pos += 2;
                }
                continue;
            }

            // FRMS tag (SNDH v2.2) - frame counts per subtune
            if &tag[0..4] == b"FRMS" {
                pos += 4;
                // Read 32-bit frame count for each subsong
                for _ in 0..meta.subsong_count.min(MAX_SUBSONGS) {
                    if pos + 4 > data.len() {
                        break;
                    }
                    let frames = ((data[pos] as u32) << 24)
                        | ((data[pos + 1] as u32) << 16)
                        | ((data[pos + 2] as u32) << 8)
                        | (data[pos + 3] as u32);
                    meta.subsong_frames.push(frames);
                    pos += 4;
                }
                continue;
            }

            // FLAG tag (SNDH v2.2) - feature flags
            if &tag[0..4] == b"FLAG" {
                pos += 4;
                // Expect '~' separator
                if pos < data.len() && data[pos] == b'~' {
                    pos += 1;
                }
                // Parse flags until we hit end or next tag
                while pos < data.len() {
                    // Skip null bytes between flags
                    while pos < data.len() && data[pos] == 0 {
                        pos += 1;
                    }
                    if pos >= data.len() {
                        break;
                    }
                    // Check for next tag (4 uppercase letters typically)
                    if pos + 4 <= data.len() {
                        let next_tag = &data[pos..pos + 4];
                        if Self::is_known_tag(next_tag) {
                            break;
                        }
                    }
                    // Apply the flag
                    let flag_char = data[pos];
                    if Self::is_flag_char(flag_char) {
                        Self::apply_flag(&mut meta.flags, flag_char);
                        pos += 1;
                    } else {
                        // Unknown character, stop parsing flags
                        break;
                    }
                }
                continue;
            }

            // #!SN tag - subtune names with word offsets
            if &tag[0..4] == b"#!SN" {
                pos += 4;
                // Align to even boundary if needed
                if pos & 1 != 0 {
                    pos += 1;
                }
                let base_pos = pos;
                // Read offset table
                let mut offsets = Vec::new();
                for _ in 0..meta.subsong_count.min(MAX_SUBSONGS) {
                    if pos + 2 > data.len() {
                        break;
                    }
                    let offset = ((data[pos] as usize) << 8) | (data[pos + 1] as usize);
                    offsets.push(offset);
                    pos += 2;
                }
                // Read names using offsets (relative to base_pos)
                for offset in offsets {
                    let name_pos = base_pos + offset;
                    if name_pos < data.len() {
                        let (name, _) = Self::read_nt_string(data, name_pos);
                        meta.subtune_names.push(name);
                    }
                }
                // Skip past all name data to next tag
                // Find HDNS or next known tag
                while pos + 4 <= data.len() {
                    if Self::is_known_tag(&data[pos..pos + 4]) {
                        break;
                    }
                    pos += 1;
                }
                continue;
            }

            // 2-character tags
            if &tag[0..2] == b"##" {
                // Subsong count
                let count_str = String::from_utf8_lossy(&tag[2..4]);
                if let Ok(count) = count_str.trim().parse::<usize>() {
                    meta.subsong_count = if count > 0 { count } else { 1 };
                }
                pos += 4;
                continue;
            }

            if &tag[0..2] == b"!#" {
                // Default subsong
                pos += 2;
                let (s, new_pos) = Self::read_nt_string(data, pos);
                if let Ok(default) = s.parse::<usize>() {
                    meta.default_subsong = default;
                }
                pos = new_pos;
                continue;
            }

            // Timer tags (TA, TB, TC, TD) or VBL (!V)
            if &tag[0..2] == b"TA"
                || &tag[0..2] == b"TB"
                || &tag[0..2] == b"TC"
                || &tag[0..2] == b"TD"
            {
                meta.timer_used = Some(tag[1] as char);
                pos += 2;
                let (s, new_pos) = Self::read_nt_string(data, pos);
                if let Ok(rate) = s.parse::<u32>() {
                    meta.player_rate = rate;
                }
                pos = new_pos;
                continue;
            }

            if &tag[0..2] == b"!V" {
                pos += 2;
                let (s, new_pos) = Self::read_nt_string(data, pos);
                if let Ok(rate) = s.parse::<u32>() {
                    meta.player_rate = rate;
                }
                pos = new_pos;
                continue;
            }

            // Unknown tag - advance by 1
            pos += 1;
        }

        // Validate default subsong
        if meta.default_subsong > meta.subsong_count || meta.default_subsong < 1 {
            meta.default_subsong = 1;
        }

        Ok(meta)
    }

    /// Read a null-terminated string from data.
    fn read_nt_string(data: &[u8], start: usize) -> (String, usize) {
        let mut end = start;
        while end < data.len() && data[end] != 0 {
            end += 1;
        }
        let s = String::from_utf8_lossy(&data[start..end]).to_string();
        (s, end + 1) // +1 to skip the null terminator
    }

    /// Check if a character is a valid FLAG character.
    fn is_flag_char(c: u8) -> bool {
        matches!(
            c,
            b'a' | b'b'
                | b'c'
                | b'd'
                | b'e'
                | b'f'
                | b'g'
                | b'h'
                | b'j'
                | b'k'
                | b'l'
                | b'p'
                | b's'
                | b'x'
                | b'y'
                | b'0'..=b'9' | b'A' | b'B' | b'C' | b'F' | b'S'
        )
    }

    /// Apply a flag character to the flags struct.
    fn apply_flag(flags: &mut SndhFlags, c: u8) {
        match c {
            b'a' => flags.timer_a = true,
            b'b' => flags.timer_b = true,
            b'c' => flags.timer_c = true,
            b'd' => flags.timer_d = true,
            b'e' => flags.ste = true,
            b'f' | b'x' => flags.sfx = true,
            b'g' => flags.digital = true,
            b'h' => flags.hbl = true,
            b'j' => flags.jingles = true,
            b'k' => flags.kill_system = true,
            b'l' => flags.lmc = true,
            b'p' => flags.aga = true,
            b's' => flags.dsp = true,
            b'y' => flags.ym2149 = true,
            // DMA sample rates (STE)
            b'0' => flags.dma_rate = Some(DmaSampleRate::Rate6258),
            b'1' => flags.dma_rate = Some(DmaSampleRate::Rate12517),
            b'2' => flags.dma_rate = Some(DmaSampleRate::Rate25033),
            b'3' => flags.dma_rate = Some(DmaSampleRate::Rate50066),
            // DMA sample rates (Falcon)
            b'4' => flags.dma_rate = Some(DmaSampleRate::Rate12292Falcon),
            b'5' => flags.dma_rate = Some(DmaSampleRate::Rate14049Falcon),
            b'6' => flags.dma_rate = Some(DmaSampleRate::Rate16390Falcon),
            b'7' => flags.dma_rate = Some(DmaSampleRate::Rate19668Falcon),
            b'8' => flags.dma_rate = Some(DmaSampleRate::Rate24585Falcon),
            b'9' => flags.dma_rate = Some(DmaSampleRate::Rate32780Falcon),
            b'A' => flags.dma_rate = Some(DmaSampleRate::Rate49170Falcon),
            // Other flags
            b'B' => flags.blitter = true,
            b'C' => flags.cpu_68020 = true,
            b'F' => flags.filters = true,
            b'S' => flags.stereo = true,
            _ => {}
        }
    }

    /// Check if bytes match a known SNDH tag.
    fn is_known_tag(tag: &[u8]) -> bool {
        if tag.len() < 4 {
            return false;
        }
        matches!(
            &tag[0..4],
            b"SNDH"
                | b"HDNS"
                | b"TITL"
                | b"COMM"
                | b"YEAR"
                | b"RIPP"
                | b"CONV"
                | b"TIME"
                | b"FRMS"
                | b"FLAG"
                | b"#!SN"
        ) || matches!(
            &tag[0..2],
            b"##" | b"!#" | b"TA" | b"TB" | b"TC" | b"TD" | b"!V"
        )
    }

    /// Get information about a specific subsong.
    ///
    /// Duration is determined from FRMS tag (frame count) if available (SNDH v2.2),
    /// falling back to TIME tag (seconds) for older files.
    pub fn get_subsong_info(&self, subsong_id: usize, sample_rate: u32) -> Option<SubsongInfo> {
        if subsong_id < 1 || subsong_id > self.metadata.subsong_count {
            return None;
        }

        let idx = subsong_id - 1;

        // Prefer FRMS (frame count, SNDH v2.2) over TIME (seconds, legacy)
        let tick_count = if let Some(&frames) = self.metadata.subsong_frames.get(idx) {
            // FRMS provides exact frame count (0 = endless loop)
            frames
        } else if let Some(&duration) = self.metadata.subsong_durations.get(idx) {
            // TIME fallback: convert seconds to frames
            (duration as u32) * self.metadata.player_rate
        } else {
            0 // Unknown duration
        };

        let samples_per_tick = sample_rate / self.metadata.player_rate;

        // Get subtune name if available
        let subtune_name = self.metadata.subtune_names.get(idx).cloned();

        Some(SubsongInfo {
            subsong_count: self.metadata.subsong_count,
            player_tick_count: tick_count,
            player_tick_rate: self.metadata.player_rate,
            samples_per_tick,
            title: self.metadata.title.clone(),
            author: self.metadata.author.clone(),
            year: self.metadata.year.clone(),
            subtune_name,
        })
    }

    /// Get raw data for uploading to Atari machine memory.
    pub fn raw_data(&self) -> &[u8] {
        &self.data
    }

    /// Get raw data size.
    pub fn raw_size(&self) -> usize {
        self.data.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_minimal_sndh() -> Vec<u8> {
        let mut data = vec![0u8; 32];
        data[0] = 0x60; // BRA.s
        data[1] = 0x1E; // offset to byte 32
        data[12..16].copy_from_slice(b"SNDH");
        data[16..20].copy_from_slice(b"HDNS");
        data
    }

    #[test]
    fn test_parse_minimal_sndh() {
        let data = make_minimal_sndh();
        let sndh = SndhFile::parse(&data).unwrap();
        assert_eq!(sndh.metadata.subsong_count, 1);
        assert_eq!(sndh.metadata.player_rate, 50);
    }

    #[test]
    fn test_invalid_magic() {
        let mut data = make_minimal_sndh();
        data[12..16].copy_from_slice(b"XXXX");
        assert!(SndhFile::parse(&data).is_err());
    }

    #[test]
    fn test_missing_bra() {
        let mut data = make_minimal_sndh();
        data[0] = 0x00;
        assert!(SndhFile::parse(&data).is_err());
    }

    fn make_sndh_with_tags(tags: &[u8]) -> Vec<u8> {
        // Calculate total size (header + tags + HDNS)
        let total_size = 16 + tags.len() + 4;
        let mut data = vec![0u8; total_size];
        data[0] = 0x60; // BRA.s
        data[1] = (total_size - 2) as u8; // offset past all tags
        data[12..16].copy_from_slice(b"SNDH");
        data[16..16 + tags.len()].copy_from_slice(tags);
        data[16 + tags.len()..16 + tags.len() + 4].copy_from_slice(b"HDNS");
        data
    }

    #[test]
    fn test_parse_frms_tag() {
        // Create SNDH with ##02 (2 subsongs) followed by FRMS tag
        let mut tags = Vec::new();
        tags.extend_from_slice(b"##02"); // 2 subsongs
        tags.extend_from_slice(b"FRMS");
        // Frame count for subsong 1: 15000 frames (big-endian)
        tags.push(0x00);
        tags.push(0x00);
        tags.push(0x3A);
        tags.push(0x98);
        // Frame count for subsong 2: 0 (endless loop)
        tags.push(0x00);
        tags.push(0x00);
        tags.push(0x00);
        tags.push(0x00);

        let data = make_sndh_with_tags(&tags);
        let sndh = SndhFile::parse(&data).unwrap();

        assert_eq!(sndh.metadata.subsong_count, 2);
        assert_eq!(sndh.metadata.subsong_frames.len(), 2);
        assert_eq!(sndh.metadata.subsong_frames[0], 15000);
        assert_eq!(sndh.metadata.subsong_frames[1], 0); // endless loop

        // Test get_subsong_info uses FRMS
        let info = sndh.get_subsong_info(1, 44100).unwrap();
        assert_eq!(info.player_tick_count, 15000);
    }

    #[test]
    fn test_parse_time_fallback() {
        // Create SNDH with TIME tag (legacy format, no FRMS)
        let mut tags = Vec::new();
        tags.extend_from_slice(b"##02"); // 2 subsongs
        tags.extend_from_slice(b"TIME");
        // Duration for subsong 1: 180 seconds (big-endian)
        tags.push(0x00);
        tags.push(0xB4);
        // Duration for subsong 2: 120 seconds
        tags.push(0x00);
        tags.push(0x78);

        let data = make_sndh_with_tags(&tags);
        let sndh = SndhFile::parse(&data).unwrap();

        assert_eq!(sndh.metadata.subsong_durations.len(), 2);
        assert_eq!(sndh.metadata.subsong_durations[0], 180);
        assert_eq!(sndh.metadata.subsong_durations[1], 120);

        // Test get_subsong_info uses TIME (converted to frames)
        let info = sndh.get_subsong_info(1, 44100).unwrap();
        assert_eq!(info.player_tick_count, 180 * 50); // 50 Hz default rate
    }

    #[test]
    fn test_frms_takes_priority_over_time() {
        // Create SNDH with both TIME and FRMS tags - FRMS should take priority
        let mut tags = Vec::new();
        tags.extend_from_slice(b"##01"); // 1 subsong
        tags.extend_from_slice(b"TIME");
        tags.push(0x00);
        tags.push(0x3C); // 60 seconds
        tags.extend_from_slice(b"FRMS");
        tags.push(0x00);
        tags.push(0x00);
        tags.push(0x0B);
        tags.push(0xB8); // 3000 frames

        let data = make_sndh_with_tags(&tags);
        let sndh = SndhFile::parse(&data).unwrap();

        // Both should be parsed
        assert_eq!(sndh.metadata.subsong_durations[0], 60);
        assert_eq!(sndh.metadata.subsong_frames[0], 3000);

        // But get_subsong_info should prefer FRMS
        let info = sndh.get_subsong_info(1, 44100).unwrap();
        assert_eq!(info.player_tick_count, 3000);
    }

    #[test]
    fn test_parse_flag_tag() {
        // Create SNDH with FLAG tag
        let mut tags = Vec::new();
        tags.extend_from_slice(b"FLAG");
        tags.push(b'~'); // separator
        tags.push(b'a'); // Timer A
        tags.push(0);
        tags.push(b'e'); // STE
        tags.push(0);
        tags.push(b'y'); // YM2149
        tags.push(0);
        tags.push(b'S'); // Stereo
        tags.push(0);
        tags.push(b'1'); // DMA rate 12517
        tags.push(0);
        tags.push(0); // End of flags

        let data = make_sndh_with_tags(&tags);
        let sndh = SndhFile::parse(&data).unwrap();

        assert!(sndh.metadata.flags.timer_a);
        assert!(sndh.metadata.flags.ste);
        assert!(sndh.metadata.flags.ym2149);
        assert!(sndh.metadata.flags.stereo);
        assert_eq!(sndh.metadata.flags.dma_rate, Some(DmaSampleRate::Rate12517));
        // These should be false
        assert!(!sndh.metadata.flags.timer_b);
        assert!(!sndh.metadata.flags.dsp);
    }

    #[test]
    fn test_real_sndh_file() {
        // Try to load a real SNDH v2.2 file with FRMS tag
        let test_file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("examples/sndh/Buzz_Me.sndh");

        if !test_file.exists() {
            eprintln!("Test file not found: {:?}, skipping", test_file);
            return;
        }

        let data = std::fs::read(&test_file).unwrap();
        let sndh = SndhFile::parse(&data).unwrap();

        eprintln!("Title: {:?}", sndh.metadata.title);
        eprintln!("Author: {:?}", sndh.metadata.author);
        eprintln!("Year: {:?}", sndh.metadata.year);
        eprintln!("Player rate: {} Hz", sndh.metadata.player_rate);
        eprintln!("Subsong count: {}", sndh.metadata.subsong_count);
        eprintln!(
            "Subsong durations (TIME): {:?}",
            sndh.metadata.subsong_durations
        );
        eprintln!("Subsong frames (FRMS): {:?}", sndh.metadata.subsong_frames);
        eprintln!(
            "Flags: timer_a={}, timer_b={}, timer_d={}, ym2149={}",
            sndh.metadata.flags.timer_a,
            sndh.metadata.flags.timer_b,
            sndh.metadata.flags.timer_d,
            sndh.metadata.flags.ym2149
        );

        // Test duration calculation
        let frames = sndh.metadata.subsong_frames.first().copied().unwrap_or(0);
        let duration_secs = frames as f32 / sndh.metadata.player_rate as f32;
        let mins = (duration_secs / 60.0) as u32;
        let secs = (duration_secs % 60.0) as u32;
        eprintln!(
            "Duration: {} frames / {} Hz = {:.1} seconds = {}:{:02}",
            frames, sndh.metadata.player_rate, duration_secs, mins, secs
        );

        // Verify FRMS is parsed
        assert!(
            !sndh.metadata.subsong_frames.is_empty(),
            "FRMS should be parsed"
        );
        assert_eq!(
            sndh.metadata.subsong_frames[0], 11565,
            "Expected 11565 frames"
        );

        // The file should have metadata
        assert!(sndh.metadata.title.is_some() || sndh.metadata.author.is_some());
    }

    #[test]
    fn test_sndh_player_duration() {
        use crate::SndhPlayer;
        use ym2149_common::ChiptunePlayerBase;

        let test_file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("examples/sndh/Buzz_Me.sndh");

        if !test_file.exists() {
            eprintln!("Test file not found: {:?}, skipping", test_file);
            return;
        }

        let data = std::fs::read(&test_file).unwrap();
        let mut player = SndhPlayer::new(&data, 44100).unwrap();
        player.init_subsong(1).unwrap();

        let duration = player.duration_seconds();
        let total_frames = player.total_frames();
        let progress = player.progress();

        eprintln!("Player duration_seconds(): {:.2}", duration);
        eprintln!("Player total_frames(): {}", total_frames);
        eprintln!("Player progress() at start: {:.2}", progress);

        // Duration should be ~231 seconds (3:51)
        assert!(
            duration > 230.0 && duration < 233.0,
            "Duration should be ~231 seconds, got {}",
            duration
        );
        assert_eq!(total_frames, 11565, "Total frames should be 11565");
        // Progress should be very close to 0 at start (init may advance a tiny bit)
        assert!(
            progress < 0.001,
            "Progress at start should be near 0.0, got {}",
            progress
        );
    }

    #[test]
    fn test_sndh_seek() {
        use crate::SndhPlayer;
        use ym2149_common::ChiptunePlayerBase;

        let test_file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("examples/sndh/Buzz_Me.sndh");

        if !test_file.exists() {
            eprintln!("Test file not found, skipping");
            return;
        }

        let data = std::fs::read(&test_file).unwrap();
        let mut player = SndhPlayer::new(&data, 44100).unwrap();
        player.init_subsong(1).unwrap();

        eprintln!("Before seek:");
        eprintln!("  duration_seconds: {}", player.duration_seconds());
        eprintln!("  playback_position: {}", player.playback_position());
        eprintln!("  current_frame: {}", player.current_frame());
        eprintln!("  total_frames: {}", player.total_frames());

        // Try to seek to 50%
        let result = player.seek(0.5);
        eprintln!("Seek to 50% result: {}", result);

        eprintln!("After seek:");
        eprintln!("  playback_position: {}", player.playback_position());
        eprintln!("  current_frame: {}", player.current_frame());

        assert!(result, "Seek should succeed");
        assert!(
            player.playback_position() > 0.4 && player.playback_position() < 0.6,
            "Position should be around 50%, got {}",
            player.playback_position()
        );
    }
}
