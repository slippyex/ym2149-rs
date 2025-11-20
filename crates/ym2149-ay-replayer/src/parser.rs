//! AY file parser producing structured [`AyFile`] data.

use crate::error::{AyError, Result};
use crate::format::{AyBlock, AyFile, AyHeader, AyPoints, AySong, AySongData};

/// Parse an AY container from raw bytes.
pub fn load_ay(data: &[u8]) -> Result<AyFile> {
    AyParser { data }.parse()
}

struct AyParser<'a> {
    data: &'a [u8],
}

impl<'a> AyParser<'a> {
    fn parse(&self) -> Result<AyFile> {
        if self.data.len() < 20 {
            return Err(AyError::UnexpectedEof);
        }

        if &self.data[0..4] != b"ZXAY" {
            return Err(AyError::InvalidFileId);
        }

        let type_id = &self.data[4..8];
        if type_id != b"EMUL" {
            let typ = String::from_utf8_lossy(type_id).to_string();
            return Err(AyError::UnsupportedType { typ });
        }

        let file_version = self.read_u16(8)?;
        let player_version = self.read_u8(10)?;
        let special_player_flag = self.read_u8(11)?;
        let author = self.read_string_pointer(12)?;
        let misc = self.read_string_pointer(14)?;

        let raw_song_count = self.read_u8(16)?;
        let raw_first_song = self.read_u8(17)?;
        let songs_ptr = self.read_required_pointer(18)?;

        let song_count = raw_song_count
            .checked_add(1)
            .ok_or_else(|| AyError::InvalidData {
                msg: "NumOfSongs overflow".to_string(),
            })?;

        if raw_first_song >= song_count {
            return Err(AyError::InvalidData {
                msg: format!(
                    "first song index {} outside available {} songs",
                    raw_first_song + 1,
                    song_count
                ),
            });
        }

        let header = AyHeader {
            file_version,
            player_version,
            special_player_flag,
            author,
            misc,
            song_count,
            first_song_index: raw_first_song,
        };

        let songs = self.parse_song_structures(song_count as usize, songs_ptr)?;

        Ok(AyFile { header, songs })
    }

    fn parse_song_structures(&self, count: usize, base_offset: usize) -> Result<Vec<AySong>> {
        let mut songs = Vec::with_capacity(count);
        for idx in 0..count {
            let entry_offset =
                base_offset
                    .checked_add(idx * 4)
                    .ok_or_else(|| AyError::InvalidData {
                        msg: "song structure offset overflow".to_string(),
                    })?;
            self.ensure_range(entry_offset, 4)?;

            let name_ptr =
                self.resolve_optional_pointer(entry_offset, self.read_i16(entry_offset)?)?;
            let song_name = if let Some(ptr) = name_ptr {
                self.read_nt_string(ptr)?
            } else {
                format!("Song {}", idx + 1)
            };

            let data_offset = self
                .resolve_pointer(entry_offset + 2, self.read_i16(entry_offset + 2)?)?
                .ok_or(AyError::MissingPointer {
                    offset: entry_offset + 2,
                })?;

            let data = self.parse_song_data(data_offset)?;
            songs.push(AySong {
                name: song_name,
                data,
            });
        }
        Ok(songs)
    }

    fn parse_song_data(&self, offset: usize) -> Result<AySongData> {
        self.ensure_range(offset, 14)?;
        let channel_map = [
            self.read_u8(offset)?,
            self.read_u8(offset + 1)?,
            self.read_u8(offset + 2)?,
            self.read_u8(offset + 3)?,
        ];
        let song_length = self.read_u16(offset + 4)?;
        let fade_length = self.read_u16(offset + 6)?;
        let hi_reg = self.read_u8(offset + 8)?;
        let lo_reg = self.read_u8(offset + 9)?;
        let points_ptr = self.resolve_optional_pointer(offset + 10, self.read_i16(offset + 10)?)?;
        let addresses_ptr =
            self.resolve_optional_pointer(offset + 12, self.read_i16(offset + 12)?)?;

        let points = match points_ptr {
            Some(ptr) => Some(self.parse_points(ptr)?),
            None => None,
        };

        let blocks = match addresses_ptr {
            Some(ptr) => self.parse_blocks(ptr)?,
            None => Vec::new(),
        };

        Ok(AySongData {
            channel_map,
            song_length_50hz: song_length,
            fade_length_50hz: fade_length,
            hi_reg,
            lo_reg,
            points,
            blocks,
        })
    }

    fn parse_points(&self, offset: usize) -> Result<AyPoints> {
        self.ensure_range(offset, 6)?;
        Ok(AyPoints {
            stack: self.read_u16(offset)?,
            init: self.read_u16(offset + 2)?,
            interrupt: self.read_u16(offset + 4)?,
        })
    }

    fn parse_blocks(&self, mut offset: usize) -> Result<Vec<AyBlock>> {
        let mut blocks = Vec::new();
        loop {
            if offset + 2 > self.data.len() {
                return Err(AyError::UnterminatedBlockTable { offset });
            }

            let address = self.read_u16(offset)?;
            if address == 0 {
                break;
            }

            self.ensure_range(offset, 6)?;
            let raw_length = self.read_u16(offset + 2)?;
            let trimmed_length = self.trim_block_length(address, raw_length);
            let data_ptr = self
                .resolve_pointer(offset + 4, self.read_i16(offset + 4)?)?
                .ok_or(AyError::MissingPointer { offset: offset + 4 })?;

            let (data, actual_len) = self.read_block_payload(data_ptr, trimmed_length)?;
            blocks.push(AyBlock {
                address,
                length: actual_len,
                data,
            });

            offset += 6;
        }

        Ok(blocks)
    }

    fn read_block_payload(&self, start: usize, requested_len: u16) -> Result<(Vec<u8>, u16)> {
        if start >= self.data.len() {
            return Err(AyError::PointerOutOfRange { offset: start });
        }
        let available = (self.data.len() - start).min(requested_len as usize);
        let end = start + available;
        Ok((self.data[start..end].to_vec(), available as u16))
    }

    fn read_string_pointer(&self, offset: usize) -> Result<String> {
        let rel = self.read_i16(offset)?;
        match self.resolve_optional_pointer(offset, rel)? {
            Some(ptr) => self.read_nt_string(ptr),
            None => Ok(String::new()),
        }
    }

    fn trim_block_length(&self, address: u16, length: u16) -> u16 {
        let max_len = 0x10000u32
            .saturating_sub(address as u32)
            .min(u16::MAX as u32);
        length.min(max_len as u16)
    }

    fn read_u8(&self, offset: usize) -> Result<u8> {
        self.ensure_range(offset, 1)?;
        Ok(self.data[offset])
    }

    fn read_u16(&self, offset: usize) -> Result<u16> {
        self.ensure_range(offset, 2)?;
        Ok(u16::from_be_bytes([
            self.data[offset],
            self.data[offset + 1],
        ]))
    }

    fn read_i16(&self, offset: usize) -> Result<i16> {
        self.ensure_range(offset, 2)?;
        Ok(i16::from_be_bytes([
            self.data[offset],
            self.data[offset + 1],
        ]))
    }

    fn read_required_pointer(&self, offset: usize) -> Result<usize> {
        let rel = self.read_i16(offset)?;
        self.resolve_pointer(offset, rel)?
            .ok_or(AyError::MissingPointer { offset })
    }

    fn resolve_pointer(&self, origin: usize, rel: i16) -> Result<Option<usize>> {
        if rel == 0 {
            return Ok(None);
        }
        let target = origin as isize + rel as isize;
        if target < 0 || target >= self.data.len() as isize {
            return Err(AyError::PointerOutOfRange { offset: origin });
        }
        Ok(Some(target as usize))
    }

    fn resolve_optional_pointer(&self, origin: usize, rel: i16) -> Result<Option<usize>> {
        self.resolve_pointer(origin, rel)
    }

    fn read_nt_string(&self, start: usize) -> Result<String> {
        if start >= self.data.len() {
            return Err(AyError::PointerOutOfRange { offset: start });
        }
        let mut end = start;
        while end < self.data.len() && self.data[end] != 0 {
            end += 1;
        }
        if end >= self.data.len() {
            return Err(AyError::UnterminatedString { start });
        }
        Ok(String::from_utf8_lossy(&self.data[start..end]).to_string())
    }

    fn ensure_range(&self, offset: usize, size: usize) -> Result<()> {
        let end = offset
            .checked_add(size)
            .ok_or_else(|| AyError::InvalidData {
                msg: "integer overflow while checking bounds".to_string(),
            })?;
        if end > self.data.len() {
            return Err(AyError::UnexpectedEof);
        }
        Ok(())
    }
}
