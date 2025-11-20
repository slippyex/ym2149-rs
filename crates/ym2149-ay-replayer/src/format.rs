//! Data structures describing parsed AY files.

/// Parsed AY file with header information and available songs.
#[derive(Debug, Clone)]
pub struct AyFile {
    /// Header metadata.
    pub header: AyHeader,
    /// All song entries contained in the file.
    pub songs: Vec<AySong>,
}

/// AY file header metadata.
#[derive(Debug, Clone)]
pub struct AyHeader {
    /// Container format version.
    pub file_version: u16,
    /// Requested AY player version (as stored in the file).
    pub player_version: u8,
    /// Optional flag reserved for special Amiga players (unused for EMUL).
    pub special_player_flag: u8,
    /// Author string extracted from the file.
    pub author: String,
    /// Misc/notes string extracted from the file.
    pub misc: String,
    /// Total number of songs contained in the file.
    pub song_count: u8,
    /// Zero-based index of the first song (stored as `FirstSong` minus one).
    pub first_song_index: u8,
}

/// AY song entry.
#[derive(Debug, Clone)]
pub struct AySong {
    /// Song title as stored in the AY file.
    pub name: String,
    /// Parsed song data required for playback.
    pub data: AySongData,
}

/// Metadata and PSG/memory layout for a single AY song.
#[derive(Debug, Clone)]
pub struct AySongData {
    /// Channel routing (Amiga channel order A/B/C/Noise).
    pub channel_map: [u8; 4],
    /// Declared song length in 1/50s units (0 when unknown).
    pub song_length_50hz: u16,
    /// Declared fade length in 1/50s units.
    pub fade_length_50hz: u16,
    /// Common register high-byte initialization value.
    pub hi_reg: u8,
    /// Common register low-byte initialization value.
    pub lo_reg: u8,
    /// Stack/INIT/INT pointers.
    pub points: Option<AyPoints>,
    /// Memory blocks that must be loaded into the Z80 address space.
    pub blocks: Vec<AyBlock>,
}

/// Z80 register setup extracted from the Points structure.
#[derive(Debug, Clone)]
pub struct AyPoints {
    /// Initial stack pointer.
    pub stack: u16,
    /// INIT routine entry point.
    pub init: u16,
    /// INTERRUPT routine entry point (0 when unused).
    pub interrupt: u16,
}

/// Memory block definition (address + data payload).
#[derive(Debug, Clone)]
pub struct AyBlock {
    /// Load address inside the Z80 memory map.
    pub address: u16,
    /// Effective length of the block (after trimming to 64K and file length).
    pub length: u16,
    /// Raw bytes to copy into the target address.
    pub data: Vec<u8>,
}
