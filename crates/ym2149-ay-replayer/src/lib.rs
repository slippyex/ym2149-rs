//! AY file parser and replayer utilities.
//!
//! This crate provides building blocks for loading Project AY (`.ay`) files:
//! - Robust parser that understands the ZXAY/EMUL container format
//! - Structured representation of metadata, song entries, and memory blocks
//! - (Upcoming) high-level player that can execute the embedded Z80 players

#![warn(missing_docs)]

pub mod error;
pub mod format;
mod machine;
mod parser;
pub mod player;

pub use crate::error::{AyError, Result};
pub use crate::format::{AyBlock, AyFile, AyHeader, AyPoints, AySong, AySongData};
pub use crate::parser::load_ay;
pub use crate::player::{AyMetadata, AyPlaybackState, AyPlayer};

#[cfg(test)]
mod tests {
    use super::*;

    const SPACE_MADNESS: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../ProjectAY/Spectrum/Demos/SpaceMadness.AY"
    ));
    const SHORT_MODULE: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../ProjectAY/Spectrum/Demos/Short.ay"
    ));

    #[test]
    fn parse_space_madness_metadata() {
        let ay = load_ay(SPACE_MADNESS).expect("failed to parse Space Madness");
        assert_eq!(ay.header.author, "Alberto Gonzales");
        assert!(
            ay.header.misc.starts_with("Unreleased Track"),
            "unexpected misc '{}'",
            ay.header.misc
        );
        assert_eq!(ay.header.song_count, 1);
        assert_eq!(ay.songs.len(), 1);

        let song = &ay.songs[0];
        assert_eq!(song.name, "Space Madness");
        assert_eq!(song.data.channel_map, [0, 1, 2, 3]);
        assert_eq!(song.data.song_length_50hz, 0);
        let points = song.data.points.as_ref().expect("points missing");
        assert_eq!(points.stack, 0x0000);
        assert_eq!(points.init, 0x801a);
        assert_eq!(points.interrupt, 0x80ab);
        assert!(!song.data.blocks.is_empty());
        assert_eq!(song.data.blocks[0].address, 0x8000);
    }

    #[test]
    fn parse_short_module_song_list() {
        let ay = load_ay(SHORT_MODULE).expect("failed to parse Short.ay");
        assert_eq!(ay.header.author, "Ivan Roshin, Moscow, 15.09.2000");
        assert!(
            ay.header.misc.starts_with("Non-standard note table"),
            "unexpected misc '{}'",
            ay.header.misc
        );
        assert_eq!(ay.header.song_count, ay.songs.len() as u8);
        assert_eq!(ay.songs.len(), 1);

        let song = &ay.songs[0];
        assert_eq!(song.name, "Short module for PHAT0     (ABC)");
        assert!(
            song.data.song_length_50hz > 0,
            "expected non-zero length metadata"
        );
        assert!(
            song.data.blocks.iter().all(|blk| !blk.data.is_empty()),
            "every block should have data payload"
        );
    }

    #[test]
    fn ay_player_generates_audio() {
        let (mut player, meta) =
            crate::player::AyPlayer::load_from_bytes(SPACE_MADNESS, 0).unwrap();
        assert_eq!(meta.song_name, "Space Madness");
        player.play().unwrap();
        let samples = player.generate_samples(1_764);
        assert_eq!(samples.len(), 1_764);
        // Ensure we actually produced non-zero samples after running at least one frame.
        assert!(
            samples.iter().any(|s| s.abs() > f32::EPSILON),
            "expected waveform data"
        );
    }

    const CPC_IMPACT: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../ProjectAY/CPC/Demos/impact demo 3_2.ay"
    ));

    #[test]
    fn ay_player_handles_cpc_ports() {
        let (mut player, meta) = crate::player::AyPlayer::load_from_bytes(CPC_IMPACT, 0).unwrap();
        assert!(meta.song_name.contains("Impact Demo"));
        player.play().unwrap();
        let mut buffer = [0.0f32; 1_764];
        player.generate_samples_into(&mut buffer);
        assert!(
            buffer.iter().any(|s| s.abs() > f32::EPSILON),
            "expected CPC track to output audio"
        );
    }
}
