//! ICE! 2.4 depacker implementation.
//!
//! ICE! was a popular data packer for the Atari ST. Many SNDH files are
//! compressed with ICE! to reduce file size. This module provides
//! decompression support.
//!
//! ICE! packed data can be recognized by the magic bytes "ICE!" at the
//! start of the file.
//!
//! Based on the public domain C implementation by Hans Wessels (2007).

use crate::error::{Result, SndhError};

/// ICE! magic header bytes
const ICE_MAGIC: u32 = 0x49434521; // "ICE!"

/// Check if data is ICE! 2.4 packed.
///
/// Returns true if the data starts with the "ICE!" magic header.
pub fn is_ice_packed(data: &[u8]) -> bool {
    if data.len() < 12 {
        return false;
    }
    get_u32_be(data, 0) == ICE_MAGIC
}

/// Get the packed size from ICE! header.
///
/// Does not validate the header - call `is_ice_packed` first.
pub fn ice_packed_size(data: &[u8]) -> u32 {
    get_u32_be(data, 4)
}

/// Get the original (unpacked) size from ICE! header.
///
/// Does not validate the header - call `is_ice_packed` first.
pub fn ice_original_size(data: &[u8]) -> u32 {
    get_u32_be(data, 8)
}

/// Depack ICE! 2.4 compressed data.
///
/// # Arguments
///
/// * `src` - ICE! compressed data
///
/// # Returns
///
/// Decompressed data, or error if decompression fails.
pub fn ice_depack(src: &[u8]) -> Result<Vec<u8>> {
    if !is_ice_packed(src) {
        return Err(SndhError::IceDepackError(
            "No ICE! header found".to_string(),
        ));
    }

    let packed_size = ice_packed_size(src) as usize;
    let orig_size = ice_original_size(src) as usize;

    if src.len() < packed_size {
        return Err(SndhError::IceDepackError(format!(
            "Data too short: expected {} bytes, got {}",
            packed_size,
            src.len()
        )));
    }

    if orig_size == 0 || orig_size > 16 * 1024 * 1024 {
        return Err(SndhError::IceDepackError(format!(
            "Invalid original size: {}",
            orig_size
        )));
    }

    let mut dst = vec![0u8; orig_size];
    let mut state = IceState::new(src, packed_size, &mut dst);
    state.depack()?;

    Ok(dst)
}

/// Read big-endian u32 from byte slice.
fn get_u32_be(data: &[u8], offset: usize) -> u32 {
    ((data[offset] as u32) << 24)
        | ((data[offset + 1] as u32) << 16)
        | ((data[offset + 2] as u32) << 8)
        | (data[offset + 3] as u32)
}

/// ICE depacker state machine.
struct IceState<'a> {
    /// Source data pointer (reading backwards from end)
    src: &'a [u8],
    /// Current read position in source
    src_pos: usize,
    /// Destination buffer
    dst: &'a mut [u8],
    /// Current write position in destination (writing backwards)
    dst_pos: usize,
    /// Current command byte
    cmd: u8,
    /// Bit mask for reading bits
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

    /// Main depack loop.
    fn depack(&mut self) -> Result<()> {
        // Initialize: read first bit to load cmd
        self.get_bits(1)?;

        // Fix reload: skip to valid bit position
        self.mask = 0x80;
        while (self.cmd & 1) == 0 {
            self.cmd >>= 1;
            self.mask >>= 1;
        }
        // Skip the 1 bit
        self.cmd >>= 1;

        loop {
            if self.get_bits(1)? != 0 {
                // Literal bytes
                let len = self.get_literal_length()?;
                self.copy_literal(len)?;
                if self.dst_pos == 0 {
                    return Ok(());
                }
            }

            // Sliding dictionary copy (always follows literal or runs alone)
            let (len, pos) = self.get_sld_params()?;
            self.copy_sld(len, pos)?;
            if self.dst_pos == 0 {
                return Ok(());
            }
        }
    }

    /// Get n bits from the bitstream.
    fn get_bits(&mut self, mut len: u32) -> Result<u32> {
        let mut result = 0u32;

        while len > 0 {
            result <<= 1;
            self.mask >>= 1;
            if self.mask == 0 {
                if self.src_pos == 0 {
                    return Err(SndhError::IceDepackError(
                        "Unexpected end of compressed data".to_string(),
                    ));
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

    /// Get literal copy length using variable-length encoding.
    fn get_literal_length(&mut self) -> Result<usize> {
        const LEN_BITS: [u32; 6] = [1, 2, 2, 3, 8, 15];
        const MAX_LEN: [u32; 6] = [1, 3, 3, 7, 255, 32768];
        const OFFSET: [usize; 6] = [1, 2, 5, 8, 15, 270];

        let mut table_pos = 0;
        let mut len;
        loop {
            len = self.get_bits(LEN_BITS[table_pos])?;
            if len != MAX_LEN[table_pos] {
                break;
            }
            table_pos += 1;
            if table_pos >= 6 {
                break;
            }
        }

        let len = len as usize + OFFSET[table_pos];

        // Clamp to remaining space
        Ok(len.min(self.dst_pos))
    }

    /// Copy literal bytes from source to destination.
    fn copy_literal(&mut self, len: usize) -> Result<()> {
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

    /// Get sliding dictionary copy parameters (length and position).
    fn get_sld_params(&mut self) -> Result<(usize, usize)> {
        const EXTRA_BITS: [u32; 5] = [0, 0, 1, 2, 10];
        const OFFSET: [usize; 5] = [0, 1, 2, 4, 8];

        // Get length
        let mut table_pos = 0;
        while self.get_bits(1)? != 0 {
            table_pos += 1;
            if table_pos == 4 {
                break;
            }
        }
        let mut len = OFFSET[table_pos] + self.get_bits(EXTRA_BITS[table_pos])? as usize;

        // Get position
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

        // Clamp to remaining space
        let len = len.min(self.dst_pos);

        Ok((len, pos))
    }

    /// Copy from sliding dictionary (within destination buffer).
    fn copy_sld(&mut self, len: usize, pos: usize) -> Result<()> {
        let mut q = self.dst_pos + pos + 1;

        for _ in 0..len {
            if self.dst_pos == 0 {
                break;
            }
            q -= 1;
            self.dst_pos -= 1;
            // Copy from already-decompressed data
            if q < self.dst.len() {
                self.dst[self.dst_pos] = self.dst[q];
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_ice_packed() {
        // Valid ICE header
        let ice_data = b"ICE!\x00\x00\x00\x10\x00\x00\x00\x20";
        assert!(is_ice_packed(ice_data));

        // Invalid header
        let not_ice = b"SNDH\x00\x00\x00\x00";
        assert!(!is_ice_packed(not_ice));

        // Too short
        let short = b"ICE";
        assert!(!is_ice_packed(short));
    }

    #[test]
    fn test_ice_sizes() {
        let ice_data = b"ICE!\x00\x00\x01\x00\x00\x00\x02\x00";
        assert_eq!(ice_packed_size(ice_data), 0x100);
        assert_eq!(ice_original_size(ice_data), 0x200);
    }
}
