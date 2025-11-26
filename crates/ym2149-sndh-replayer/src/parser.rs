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
    /// Duration of each subsong in seconds (from TIME tag)
    pub subsong_durations: Vec<u16>,
    /// Which timer is used (A, B, C, D) - from TA/TB/TC/TD tags
    pub timer_used: Option<char>,
}

/// Information about a specific subsong
#[derive(Debug, Clone)]
pub struct SubsongInfo {
    /// Total number of subsongs
    pub subsong_count: usize,
    /// Number of ticks to play (0 = unknown/infinite)
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
                // Read 16-bit duration for each subsong
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

            if &tag[0..4] == b"!#SN" {
                // Subsong name offsets - skip them
                pos += 4 + meta.subsong_count * 2;
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

    /// Get information about a specific subsong.
    pub fn get_subsong_info(&self, subsong_id: usize, sample_rate: u32) -> Option<SubsongInfo> {
        if subsong_id < 1 || subsong_id > self.metadata.subsong_count {
            return None;
        }

        let duration = self
            .metadata
            .subsong_durations
            .get(subsong_id - 1)
            .copied()
            .unwrap_or(0);

        let tick_count = (duration as u32) * self.metadata.player_rate;
        let samples_per_tick = sample_rate / self.metadata.player_rate;

        Some(SubsongInfo {
            subsong_count: self.metadata.subsong_count,
            player_tick_count: tick_count,
            player_tick_rate: self.metadata.player_rate,
            samples_per_tick,
            title: self.metadata.title.clone(),
            author: self.metadata.author.clone(),
            year: self.metadata.year.clone(),
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
}
