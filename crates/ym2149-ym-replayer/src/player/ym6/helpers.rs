//! Helper functions for reading YM file data

use crate::Result;

/// Read big-endian u16 from byte slice
pub(in crate::player) fn read_be_u16(data: &[u8], offset: &mut usize) -> Result<u16> {
    if *offset + 2 > data.len() {
        return Err("Unexpected end of data while reading u16".into());
    }
    let value = u16::from_be_bytes([data[*offset], data[*offset + 1]]);
    *offset += 2;
    Ok(value)
}

/// Read big-endian u32 from byte slice
pub(in crate::player) fn read_be_u32(data: &[u8], offset: &mut usize) -> Result<u32> {
    if *offset + 4 > data.len() {
        return Err("Unexpected end of data while reading u32".into());
    }
    let value = u32::from_be_bytes([
        data[*offset],
        data[*offset + 1],
        data[*offset + 2],
        data[*offset + 3],
    ]);
    *offset += 4;
    Ok(value)
}

/// Read null-terminated C string from byte slice
pub(in crate::player) fn read_c_string(data: &[u8], offset: &mut usize) -> Result<String> {
    if *offset >= data.len() {
        return Err("Unexpected end of data while reading string".into());
    }
    let start = *offset;
    while *offset < data.len() && data[*offset] != 0 {
        *offset += 1;
    }
    if *offset >= data.len() {
        return Err("Unterminated string in YM tracker data".into());
    }
    let string = String::from_utf8_lossy(&data[start..*offset]).to_string();
    *offset += 1; // Skip null terminator
    Ok(string)
}
