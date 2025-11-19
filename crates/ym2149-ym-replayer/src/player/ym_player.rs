//! YM6 File Player
//!
//! Plays back YM2-YM6 format chiptune files with proper VBL synchronization.

use super::effects_pipeline::EffectsPipeline;
use super::format_profile::{FormatMode, FormatProfile, create_profile};
use super::frame_sequencer::FrameSequencer;
use super::tracker_player::TrackerState;
use super::ym6::{LoadSummary, Ym6Info};
use super::{PlaybackState, VblSync};
use crate::Result;
use ym2149::{Ym2149, Ym2149Backend};

/// Generic YM6 File Player
///
/// This player is generic over any YM2149 backend implementation, allowing flexibility
/// in choosing between hardware-accurate emulation and experimental synthesizers.
///
/// Type alias [`Ym6Player`] provides the default concrete type using hardware-accurate Ym2149.
pub struct Ym6PlayerGeneric<B: Ym2149Backend> {
    /// PSG chip backend
    pub(in crate::player) chip: B,
    /// VBL synchronization
    pub(in crate::player) vbl: VblSync,
    /// Playback state
    pub(in crate::player) state: PlaybackState,
    /// Frame sequencer handling register frames and timing
    pub(in crate::player) sequencer: FrameSequencer,
    /// Song metadata
    pub(in crate::player) info: Option<Ym6Info>,
    /// Digidrum sample bank (raw bytes from file)
    pub(in crate::player) digidrums: Vec<Vec<u8>>,
    /// YM6 attributes bitfield (A_* flags)
    pub(in crate::player) attributes: u32,
    /// Format-specific behavior adapter
    pub(in crate::player) format_profile: Box<dyn FormatProfile>,
    /// Effects manager for YM6 special effects
    pub(in crate::player) effects: EffectsPipeline,
    /// Tracker playback state (for YMT1/YMT2 formats)
    pub(in crate::player) tracker: Option<TrackerState>,
    /// Indicates if current song uses tracker mixing path
    pub(in crate::player) is_tracker_mode: bool,
    /// Flag to track if first frame's registers have been pre-loaded
    pub(in crate::player) first_frame_pre_loaded: bool,
    /// Cache previous R13 (envelope shape) to avoid redundant resets
    pub(in crate::player) prev_r13: Option<u8>,
}

/// Concrete YM6 player using hardware-accurate Ym2149 emulation
///
/// This is the default player type that provides full YM6 compatibility
/// including special effects (SID, Sync Buzzer, DigiDrums).
pub type Ym6Player = Ym6PlayerGeneric<Ym2149>;

impl<B: Ym2149Backend> Ym6PlayerGeneric<B> {
    /// Create a new YM6 player with empty song
    pub fn new() -> Self {
        Ym6PlayerGeneric {
            chip: B::new(),
            vbl: VblSync::default(),
            state: PlaybackState::Stopped,
            sequencer: FrameSequencer::new(),
            info: None,
            digidrums: Vec::new(),
            attributes: 0,
            format_profile: create_profile(FormatMode::Basic),
            effects: EffectsPipeline::new(44_100),
            tracker: None,
            is_tracker_mode: false,
            first_frame_pre_loaded: false,
            prev_r13: None,
        }
    }

    /// Mute or unmute a channel (0=A,1=B,2=C)
    pub fn set_channel_mute(&mut self, channel: usize, mute: bool) {
        self.chip.set_channel_mute(channel, mute);
    }

    /// Check if a channel is muted
    pub fn is_channel_muted(&self, channel: usize) -> bool {
        self.chip.is_channel_muted(channel)
    }
}

impl<B: Ym2149Backend> Default for Ym6PlayerGeneric<B> {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience helper to create and load a player from YM data.
///
/// Uses the default hardware-accurate Ym2149 backend.
pub fn load_song(data: &[u8]) -> Result<(Ym6Player, LoadSummary)> {
    let mut player = Ym6Player::new();
    let summary = player.load_data(data)?;
    Ok((player, summary))
}

/// Type alias preserving the legacy `Player` name.
pub type Player = Ym6Player;

// Hardware-specific methods only available for the concrete Ym2149 backend
impl Ym6Player {
    /// Get mutable access to the underlying Ym2149 chip
    ///
    /// This allows direct manipulation of chip registers for advanced use cases.
    /// Only available when using the hardware-accurate Ym2149 backend.
    pub fn get_chip_mut(&mut self) -> &mut Ym2149 {
        &mut self.chip
    }

    /// Get read-only access to the underlying Ym2149 chip
    ///
    /// Only available when using the hardware-accurate Ym2149 backend.
    pub fn get_chip(&self) -> &Ym2149 {
        &self.chip
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::{PlaybackController, YmFileFormat};

    #[test]
    fn test_ym6_player_creation() {
        let player = Ym6Player::new();
        assert_eq!(player.state, PlaybackState::Stopped);
        assert_eq!(player.frame_count(), 0);
    }

    #[test]
    fn test_load_data_detects_ym3() {
        let mut data = Vec::new();
        data.extend_from_slice(b"YM3!");
        data.extend_from_slice(&[0u8; 14 * 2]);

        let mut player = Ym6Player::new();
        let summary = player.load_data(&data).expect("YM3 load failed");

        assert_eq!(summary.format, YmFileFormat::Ym3);
        assert_eq!(summary.frame_count, 2);

        player.play().unwrap();
        let samples_needed = summary.samples_per_frame as usize * summary.frame_count;
        let _ = player.generate_samples(samples_needed + 1);
        assert_eq!(player.state(), PlaybackState::Stopped);
    }

    #[test]
    fn test_load_data_detects_ym3b_loop() {
        let mut data = Vec::new();
        data.extend_from_slice(b"YM3b");
        data.extend_from_slice(&[0u8; 14 * 2]);
        // Loop back to frame 1 (second frame)
        data.extend_from_slice(&1u32.to_be_bytes());

        let mut player = Ym6Player::new();
        let summary = player.load_data(&data).expect("YM3b load failed");

        assert_eq!(summary.format, YmFileFormat::Ym3b);
        assert_eq!(summary.frame_count, 2);

        player.play().unwrap();
        let samples_needed = summary.samples_per_frame as usize * summary.frame_count * 3;
        let _ = player.generate_samples(samples_needed);
        assert_eq!(player.state(), PlaybackState::Playing);
        assert!(player.get_current_frame() < summary.frame_count);
    }

    #[test]
    fn test_ym6_player_initialization() {
        // Test that a new player initializes with correct default state
        let player = Ym6Player::new();
        assert_eq!(player.frame_count(), 0, "New player should have 0 frames");
        assert_eq!(
            player.get_current_frame(),
            0,
            "New player should start at frame 0"
        );
        assert_eq!(
            player.state(),
            PlaybackState::Stopped,
            "New player should be stopped"
        );
    }

    #[test]
    fn test_ym6_player_frame_progression() {
        // Test that frame position advances correctly during playback
        let mut player = Ym6Player::new();
        // Create 10 frames of test data (16 bytes per frame for YM6)
        let test_frames = vec![[0x00u8; 16]; 10];
        player.load_frames(test_frames);

        player.play().unwrap();
        assert_eq!(player.state(), PlaybackState::Playing);

        // Advance through several frames
        let _ = player.generate_samples(4410); // ~0.1 seconds at 44.1kHz
        assert!(
            player.get_current_frame() <= 10,
            "Frame position should not exceed frame count"
        );
    }

    #[test]
    fn test_ym6_player_load_frames() {
        let mut player = Ym6Player::new();
        let frames = vec![[0u8; 16]; 10];
        player.load_frames(frames);
        assert_eq!(player.frame_count(), 10);
    }

    #[test]
    fn test_ym6_player_playback() {
        let mut player = Ym6Player::new();
        let frames = vec![[0u8; 16]; 5];
        player.load_frames(frames);
        player.play().unwrap();

        let samples = player.generate_samples(100);
        assert_eq!(samples.len(), 100);
    }

    #[test]
    fn test_ym6_player_duration() {
        let mut player = Ym6Player::new();
        let frames = vec![[0u8; 16]; 250]; // 250 frames at 50Hz = 5 seconds
        player.load_frames(frames);
        let duration = player.get_duration_seconds();
        assert!(duration > 4.9 && duration < 5.1);
    }

    #[test]
    fn test_ym6_player_looping() {
        let mut player = Ym6Player::new();
        let frames = vec![[0x42u8; 16]; 10];
        player.load_frames(frames);
        player.set_loop_frame(5);
        player.play().unwrap();

        // Generate enough samples to reach end and loop
        // Need more than 10 * 882 samples to reach end, then generate more
        let _ = player.generate_samples(10000);

        // After looping, we should be at or past frame 5
        // The exact frame depends on timing, so just check we're in the loop range
        assert!(player.get_current_frame() >= 5 && player.get_current_frame() < 10);
        assert_eq!(player.state, PlaybackState::Playing);
    }

    #[test]
    fn test_ym6_player_position() {
        let mut player = Ym6Player::new();
        let frames = vec![[0u8; 16]; 100];
        player.load_frames(frames);
        player.play().unwrap();

        let pos = player.get_playback_position();
        assert!((0.0..=1.0).contains(&pos));
    }

    #[test]
    fn test_ym6_player_load_ym6_with_metadata() {
        // Create a simple YM6 file with metadata
        let mut ym6_data = Vec::new();

        // Header
        ym6_data.extend_from_slice(b"YM6!"); // Magic (4 bytes)
        ym6_data.extend_from_slice(b"LeOnArD!"); // Signature (8 bytes)
        ym6_data.extend_from_slice(&(2u32).to_be_bytes()); // Frame count (4 bytes)
        ym6_data.extend_from_slice(&0u32.to_be_bytes()); // Attributes (4 bytes)
        ym6_data.extend_from_slice(&0u16.to_be_bytes()); // Digidrum count (2 bytes)
        ym6_data.extend_from_slice(&2000000u32.to_be_bytes()); // Master clock (4 bytes)
        ym6_data.extend_from_slice(&50u16.to_be_bytes()); // Frame rate (2 bytes)
        ym6_data.extend_from_slice(&0u32.to_be_bytes()); // Loop frame (4 bytes)
        ym6_data.extend_from_slice(&0u16.to_be_bytes()); // Extra data size (2 bytes)

        // Metadata: song name
        ym6_data.extend_from_slice(b"Test Song\0");

        // Metadata: author
        ym6_data.extend_from_slice(b"Test Author\0");

        // Metadata: comment
        ym6_data.extend_from_slice(b"Test Comment\0");

        // Frame data (2 frames, 16 bytes each)
        ym6_data.extend_from_slice(&[0u8; 16]);
        ym6_data.extend_from_slice(&[1u8; 16]);

        // End marker
        ym6_data.extend_from_slice(b"End!");

        // Load and verify
        let mut player = Ym6Player::new();
        assert!(player.load_ym6(&ym6_data).is_ok());

        // Check metadata was populated
        let info = player.info();
        assert!(info.is_some());

        let info = info.unwrap();
        assert_eq!(info.song_name, "Test Song");
        assert_eq!(info.author, "Test Author");
        assert_eq!(info.comment, "Test Comment");
        assert_eq!(info.frame_count, 2);
        assert_eq!(info.frame_rate, 50);
        assert_eq!(info.master_clock, 2000000);

        // Check frames were loaded
        assert_eq!(player.frame_count(), 2);

        // Check samples per frame was calculated correctly
        // 44100 / 50 = 882 samples per frame
        player.play().unwrap();
        let samples = player.generate_samples(882);
        assert_eq!(samples.len(), 882);
        assert_eq!(player.get_current_frame(), 1); // Should have advanced to frame 1
    }

    #[test]
    fn test_ym6_player_duration_with_custom_frame_rate() {
        // Test with 60Hz NTSC frame rate to verify duration calculation uses actual frame rate
        let mut ym6_data = Vec::new();

        // Header
        ym6_data.extend_from_slice(b"YM6!"); // Magic (4 bytes)
        ym6_data.extend_from_slice(b"LeOnArD!"); // Signature (8 bytes)
        ym6_data.extend_from_slice(&(300u32).to_be_bytes()); // 300 frames (4 bytes)
        ym6_data.extend_from_slice(&0u32.to_be_bytes()); // Attributes (4 bytes)
        ym6_data.extend_from_slice(&0u16.to_be_bytes()); // Digidrum count (2 bytes)
        ym6_data.extend_from_slice(&2000000u32.to_be_bytes()); // Master clock (4 bytes)
        ym6_data.extend_from_slice(&60u16.to_be_bytes()); // Frame rate: 60Hz NTSC (2 bytes)
        ym6_data.extend_from_slice(&0u32.to_be_bytes()); // Loop frame (4 bytes)
        ym6_data.extend_from_slice(&0u16.to_be_bytes()); // Extra data size (2 bytes)

        // Metadata
        ym6_data.extend_from_slice(b"Test NTSC\0");
        ym6_data.extend_from_slice(b"Author\0");
        ym6_data.extend_from_slice(b"Comment\0");

        // Frame data (300 frames, 16 bytes each)
        ym6_data.extend_from_slice(&vec![0u8; 300 * 16]);

        // End marker
        ym6_data.extend_from_slice(b"End!");

        // Load and verify
        let mut player = Ym6Player::new();
        assert!(player.load_ym6(&ym6_data).is_ok());

        // Verify metadata was populated with correct frame rate
        let info = player.info().unwrap();
        assert_eq!(info.frame_rate, 60);

        // Verify duration is calculated correctly: 300 frames at 60Hz = 5.0 seconds
        let duration = player.get_duration_seconds();
        assert!(
            (duration - 5.0).abs() < 0.01,
            "Expected ~5.0s, got {}",
            duration
        );

        // Verify samples per frame was calculated for 60Hz: 44100 / 60 = 735 samples
        // Generate 735 samples (1 frame) and verify we advance to frame 1
        player.play().unwrap();
        let samples = player.generate_samples(735);
        assert_eq!(samples.len(), 735);
        assert_eq!(player.get_current_frame(), 1);
    }

    #[test]
    fn test_ym6_player_duration_default_frame_rate() {
        // Test that manually loaded frames default to 50Hz for duration calculation
        let mut player = Ym6Player::new();
        let frames = vec![[0u8; 16]; 250]; // 250 frames at 50Hz = 5.0 seconds
        player.load_frames(frames);

        let duration = player.get_duration_seconds();
        assert!(
            (duration - 5.0).abs() < 0.01,
            "Expected ~5.0s, got {}",
            duration
        );
    }

    #[test]
    fn test_sync_buzzer_enable() {
        // Test enabling Sync Buzzer effect
        let mut player = Ym6Player::new();
        let frames = vec![[0u8; 16]; 10];
        player.load_frames(frames);

        // Should succeed with valid frequency
        assert!(player.enable_sync_buzzer(6000).is_ok());

        // Verify effects manager has sync buzzer enabled
        assert!(player.effects.sync_buzzer_is_enabled());
    }

    #[test]
    fn test_sync_buzzer_disable() {
        // Test disabling Sync Buzzer effect
        let mut player = Ym6Player::new();
        let frames = vec![[0u8; 16]; 10];
        player.load_frames(frames);

        // Enable then disable
        assert!(player.enable_sync_buzzer(6000).is_ok());
        player.disable_sync_buzzer();

        // Verify effects manager has sync buzzer disabled
        assert!(!player.effects.sync_buzzer_is_enabled());
    }

    #[test]
    fn test_sync_buzzer_zero_frequency_error() {
        // Test that zero frequency is rejected
        let mut player = Ym6Player::new();
        let frames = vec![[0u8; 16]; 10];
        player.load_frames(frames);

        // Should fail with zero frequency
        let result = player.enable_sync_buzzer(0);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("frequency must be > 0")
        );
    }

    #[test]
    fn test_sync_buzzer_with_playback() {
        // Test that Sync Buzzer works during playback
        let mut player = Ym6Player::new();

        // Create a simple test frame with envelope shape 0x0F (Hold-Sawtooth)
        let mut frame = [0u8; 16];
        frame[13] = 0x0F; // Register R13: envelope shape = Hold-Sawtooth
        frame[8] = 0x0F; // Register R8: amplitude with envelope
        frame[7] = 0xBE; // Register R7: mixer - enable channel A tone

        let frames = vec![frame; 100];
        player.load_frames(frames);

        // Enable Sync Buzzer
        assert!(player.enable_sync_buzzer(6000).is_ok());

        // Play and generate some samples
        player.play().unwrap();
        let samples = player.generate_samples(1000);

        assert_eq!(samples.len(), 1000);
        // Samples should be valid (not NaN or Inf)
        for sample in samples {
            assert!(sample.is_finite());
        }
    }
}
