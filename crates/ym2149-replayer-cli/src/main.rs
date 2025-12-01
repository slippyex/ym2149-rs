//! YM2149 PSG Real-time Streaming Playback CLI
//!
//! Command-line player for YM chiptune files featuring:
//! - Real-time audio streaming with low latency
//! - Terminal-based visualization
//! - Interactive playback control
//! - YM2149 hardware emulation

mod args;
mod audio;
mod player_factory;
mod streaming;
mod visualization;
mod viz_helpers;

use audio::{DEFAULT_SAMPLE_RATE, StreamConfig};
use std::sync::Arc;
use std::time::Instant;
use ym2149::Ym2149Backend;
use ym2149_arkos_replayer::ArkosPlayer;
use ym2149_ay_replayer::{AyPlayer, CPC_UNSUPPORTED_MSG, PlaybackState as AyPlaybackState};
use ym2149_common::PlaybackState as SndhPlaybackState;
use ym2149_sndh_replayer::SndhPlayer;
use ym2149_ym_replayer::PlaybackController;
use ym2149_ym_replayer::player::ym_player::Ym6PlayerGeneric;

use args::CliArgs;
use player_factory::{create_demo_player, create_player};
use streaming::StreamingContext;
use visualization::run_visualization_loop;

/// Snapshot of chip state for visualization.
#[derive(Clone, Copy)]
pub struct VisualSnapshot {
    /// YM2149 register values (R0-R15)
    pub registers: [u8; 16],
    /// Sync buzzer effect active
    pub sync_buzzer: bool,
    /// SID voice effects active per channel
    pub sid_active: [bool; 3],
    /// Drum effects active per channel
    pub drum_active: [bool; 3],
}

/// Common trait for real-time chip emulation backends.
///
/// This trait abstracts over different YM chip implementations,
/// allowing the CLI to work with various backends.
pub trait RealtimeChip: PlaybackController + Send {
    /// Generate audio samples.
    fn generate_samples(&mut self, count: usize) -> Vec<f32>;

    /// Generate audio samples into a pre-allocated buffer (zero-allocation hot path).
    fn generate_samples_into(&mut self, buffer: &mut [f32]);

    /// Get current chip state for visualization.
    fn visual_snapshot(&self) -> VisualSnapshot;

    /// Mute or unmute a channel.
    fn set_channel_mute(&mut self, channel: usize, mute: bool);

    /// Check if a channel is muted.
    fn is_channel_muted(&self, channel: usize) -> bool;

    /// Get playback position as percentage (0.0 to 1.0).
    fn get_playback_position(&self) -> f32;

    /// Enable/disable ST color filter when supported.
    fn set_color_filter(&mut self, enabled: bool);

    /// Optional reason why playback can't continue (e.g., unsupported format).
    fn unsupported_reason(&self) -> Option<&'static str> {
        None
    }

    /// Get the number of subsongs/tracks in this file.
    /// Returns 1 for formats that don't support multiple subsongs.
    fn subsong_count(&self) -> usize {
        1
    }

    /// Get the current subsong index (1-based for SNDH, 0-based for AKS).
    /// Returns 1 for formats that don't support multiple subsongs.
    fn current_subsong(&self) -> usize {
        1
    }

    /// Switch to a different subsong. Returns true if successful.
    /// The index is 1-based for SNDH, 0-based for AKS.
    fn set_subsong(&mut self, _index: usize) -> bool {
        false
    }

    /// Check if this player supports multiple subsongs.
    fn has_subsongs(&self) -> bool {
        self.subsong_count() > 1
    }
}

impl<B: Ym2149Backend + 'static> RealtimeChip for Ym6PlayerGeneric<B> {
    fn generate_samples(&mut self, count: usize) -> Vec<f32> {
        Ym6PlayerGeneric::generate_samples(self, count)
    }

    fn generate_samples_into(&mut self, buffer: &mut [f32]) {
        Ym6PlayerGeneric::generate_samples_into(self, buffer)
    }

    fn visual_snapshot(&self) -> VisualSnapshot {
        let regs = self.dump_registers();
        let (sync, sid, drum) = self.get_active_effects();
        VisualSnapshot {
            registers: regs,
            sync_buzzer: sync,
            sid_active: sid,
            drum_active: drum,
        }
    }

    fn set_channel_mute(&mut self, channel: usize, mute: bool) {
        self.set_channel_mute(channel, mute);
    }

    fn is_channel_muted(&self, channel: usize) -> bool {
        self.is_channel_muted(channel)
    }

    fn get_playback_position(&self) -> f32 {
        Ym6PlayerGeneric::get_playback_position(self)
    }

    fn set_color_filter(&mut self, enabled: bool) {
        self.set_color_filter(enabled);
    }
}

// ArkosPlayer wrapper for CLI integration
pub struct ArkosPlayerWrapper {
    player: ArkosPlayer,
    song: Arc<ym2149_arkos_replayer::AksSong>,
    current_subsong: usize,
}

impl ArkosPlayerWrapper {
    pub fn new(player: ArkosPlayer) -> Self {
        let song = player.song();
        Self {
            player,
            song,
            current_subsong: 0,
        }
    }
}

impl PlaybackController for ArkosPlayerWrapper {
    fn play(&mut self) -> ym2149_ym_replayer::Result<()> {
        self.player
            .play()
            .map_err(|e| format!("Arkos play error: {}", e).into())
    }

    fn pause(&mut self) -> ym2149_ym_replayer::Result<()> {
        self.player
            .pause()
            .map_err(|e| format!("Arkos pause error: {}", e).into())
    }

    fn stop(&mut self) -> ym2149_ym_replayer::Result<()> {
        self.player
            .stop()
            .map_err(|e| format!("Arkos stop error: {}", e).into())
    }

    fn state(&self) -> ym2149_ym_replayer::PlaybackState {
        if self.player.is_playing() {
            ym2149_ym_replayer::PlaybackState::Playing
        } else {
            ym2149_ym_replayer::PlaybackState::Paused
        }
    }
}

impl RealtimeChip for ArkosPlayerWrapper {
    fn generate_samples(&mut self, count: usize) -> Vec<f32> {
        self.player.generate_samples(count)
    }

    fn generate_samples_into(&mut self, buffer: &mut [f32]) {
        self.player.generate_samples_into(buffer);
    }

    fn visual_snapshot(&self) -> VisualSnapshot {
        // Use first PSG chip for visualization (multi-PSG songs use chip 0 for main melody)
        let registers = self
            .player
            .chip(0)
            .map(|chip| chip.dump_registers())
            .unwrap_or([0u8; 16]);
        VisualSnapshot {
            registers,
            sync_buzzer: false,
            sid_active: [false; 3],
            drum_active: [false; 3],
        }
    }

    fn set_channel_mute(&mut self, channel: usize, mute: bool) {
        self.player.set_channel_mute(channel, mute);
    }

    fn is_channel_muted(&self, channel: usize) -> bool {
        self.player.is_channel_muted(channel)
    }

    fn get_playback_position(&self) -> f32 {
        // Calculate position as ratio of current_tick to estimated_total_ticks
        let current = self.player.current_tick_index();
        let total = self.player.estimated_total_ticks();
        if total > 0 {
            current as f32 / total as f32
        } else {
            0.0
        }
    }

    fn set_color_filter(&mut self, _enabled: bool) {
        // Not applicable for Arkos
    }

    fn subsong_count(&self) -> usize {
        self.song.subsongs.len()
    }

    fn current_subsong(&self) -> usize {
        // AKS uses 0-based indexing, return as 1-based for consistency
        self.current_subsong + 1
    }

    fn set_subsong(&mut self, index: usize) -> bool {
        // Convert 1-based input to 0-based
        let zero_based = index.saturating_sub(1);
        if zero_based < self.song.subsongs.len()
            && let Ok(new_player) = ArkosPlayer::new_from_arc(Arc::clone(&self.song), zero_based)
        {
            self.player = new_player;
            self.current_subsong = zero_based;
            let _ = self.player.play();
            return true;
        }
        false
    }
}

/// AY player wrapper for CLI integration
pub struct AyPlayerWrapper {
    player: AyPlayer,
}

impl AyPlayerWrapper {
    pub fn new(player: AyPlayer) -> Self {
        Self { player }
    }
}

impl PlaybackController for AyPlayerWrapper {
    fn play(&mut self) -> ym2149_ym_replayer::Result<()> {
        match self.player.play() {
            Ok(()) => Ok(()),
            Err(e) => {
                if self.player.requires_cpc_firmware() {
                    Err(CPC_UNSUPPORTED_MSG.into())
                } else {
                    Err(format!("AY play error: {e}").into())
                }
            }
        }
    }

    fn pause(&mut self) -> ym2149_ym_replayer::Result<()> {
        self.player.pause();
        Ok(())
    }

    fn stop(&mut self) -> ym2149_ym_replayer::Result<()> {
        self.player
            .stop()
            .map_err(|e| format!("AY stop error: {e}").into())
    }

    fn state(&self) -> ym2149_ym_replayer::PlaybackState {
        match self.player.playback_state() {
            AyPlaybackState::Playing => ym2149_ym_replayer::PlaybackState::Playing,
            AyPlaybackState::Paused => ym2149_ym_replayer::PlaybackState::Paused,
            AyPlaybackState::Stopped => ym2149_ym_replayer::PlaybackState::Stopped,
        }
    }
}

impl RealtimeChip for AyPlayerWrapper {
    fn generate_samples(&mut self, count: usize) -> Vec<f32> {
        self.player.generate_samples(count)
    }

    fn generate_samples_into(&mut self, buffer: &mut [f32]) {
        self.player.generate_samples_into(buffer);
    }

    fn visual_snapshot(&self) -> VisualSnapshot {
        VisualSnapshot {
            registers: self.player.chip().dump_registers(),
            sync_buzzer: false,
            sid_active: [false; 3],
            drum_active: [false; 3],
        }
    }

    fn set_channel_mute(&mut self, channel: usize, mute: bool) {
        self.player.set_channel_mute(channel, mute);
    }

    fn is_channel_muted(&self, channel: usize) -> bool {
        self.player.is_channel_muted(channel)
    }

    fn get_playback_position(&self) -> f32 {
        self.player.playback_position()
    }

    fn set_color_filter(&mut self, enabled: bool) {
        self.player.set_color_filter(enabled);
    }

    fn unsupported_reason(&self) -> Option<&'static str> {
        self.player
            .requires_cpc_firmware()
            .then_some(CPC_UNSUPPORTED_MSG)
    }
}

/// SNDH player wrapper for CLI integration
pub struct SndhPlayerWrapper {
    player: SndhPlayer,
}

impl SndhPlayerWrapper {
    /// Create wrapper from an existing SndhPlayer
    pub fn from_player(player: SndhPlayer) -> Self {
        Self { player }
    }

    /// Create a new SNDH player from raw file data
    pub fn new(sndh_data: &[u8], sample_rate: u32) -> Result<Self, String> {
        let mut player = SndhPlayer::new(sndh_data, sample_rate)
            .map_err(|e| format!("SNDH player init failed: {e}"))?;

        // Initialize first subsong
        player
            .init_subsong(1)
            .map_err(|e| format!("Subsong init failed: {e}"))?;

        Ok(Self { player })
    }

    /// Get player metadata
    pub fn metadata(&self) -> &ym2149_common::BasicMetadata {
        use ym2149_common::ChiptunePlayer;
        self.player.metadata()
    }
}

impl PlaybackController for SndhPlayerWrapper {
    fn play(&mut self) -> ym2149_ym_replayer::Result<()> {
        use ym2149_common::ChiptunePlayer;
        self.player.play();
        Ok(())
    }

    fn pause(&mut self) -> ym2149_ym_replayer::Result<()> {
        use ym2149_common::ChiptunePlayer;
        self.player.pause();
        Ok(())
    }

    fn stop(&mut self) -> ym2149_ym_replayer::Result<()> {
        use ym2149_common::ChiptunePlayer;
        self.player.stop();
        Ok(())
    }

    fn state(&self) -> ym2149_ym_replayer::PlaybackState {
        use ym2149_common::ChiptunePlayer;
        match self.player.state() {
            SndhPlaybackState::Playing => ym2149_ym_replayer::PlaybackState::Playing,
            SndhPlaybackState::Paused => ym2149_ym_replayer::PlaybackState::Paused,
            SndhPlaybackState::Stopped => ym2149_ym_replayer::PlaybackState::Stopped,
        }
    }
}

impl RealtimeChip for SndhPlayerWrapper {
    fn generate_samples(&mut self, count: usize) -> Vec<f32> {
        use ym2149_common::ChiptunePlayer;
        let mut buffer = vec![0.0; count];
        self.player.generate_samples_into(&mut buffer);
        buffer
    }

    fn generate_samples_into(&mut self, buffer: &mut [f32]) {
        use ym2149_common::ChiptunePlayer;
        self.player.generate_samples_into(buffer);
    }

    fn visual_snapshot(&self) -> VisualSnapshot {
        // SNDH uses native 68000 code - extract YM registers from the emulated PSG
        let regs = self.player.ym2149().dump_registers();
        VisualSnapshot {
            registers: regs,
            sync_buzzer: false,
            sid_active: [false; 3],
            drum_active: [false; 3],
        }
    }

    fn set_channel_mute(&mut self, channel: usize, mute: bool) {
        self.player.ym2149_mut().set_channel_mute(channel, mute);
    }

    fn is_channel_muted(&self, channel: usize) -> bool {
        self.player.ym2149().is_channel_muted(channel)
    }

    fn get_playback_position(&self) -> f32 {
        // SNDH doesn't have easy frame position tracking
        0.0
    }

    fn set_color_filter(&mut self, _enabled: bool) {
        // Not applicable for SNDH (uses actual 68000 code)
    }

    fn subsong_count(&self) -> usize {
        self.player.subsong_count()
    }

    fn current_subsong(&self) -> usize {
        self.player.current_subsong()
    }

    fn set_subsong(&mut self, index: usize) -> bool {
        use ym2149_common::ChiptunePlayer;
        if index >= 1
            && index <= self.player.subsong_count()
            && self.player.init_subsong(index).is_ok()
        {
            self.player.play();
            return true;
        }
        false
    }
}

fn main() -> ym2149_ym_replayer::Result<()> {
    println!("YM2149 PSG Emulator - Real-time Streaming Playback");
    println!("===================================================\n");

    // Parse command-line arguments
    let args = CliArgs::parse();

    if args.show_help {
        CliArgs::print_help();
        return if args.file_path.is_none() {
            Ok(())
        } else {
            Err("Invalid arguments".into())
        };
    }

    // Create player instance
    let player_info = match args.file_path {
        Some(ref file_path) => {
            create_player(file_path, args.chip_choice, args.color_filter_override)?
        }
        None => create_demo_player(args.chip_choice)?,
    };

    // Display file information
    println!("File Information:");
    println!("{}\n", player_info.song_info);
    println!("Selected Chip: {}\n", args.chip_choice);

    // Configure streaming
    let config = StreamConfig::low_latency(DEFAULT_SAMPLE_RATE);
    println!("Streaming Configuration:");
    println!("  Sample rate: {} Hz", config.sample_rate);
    println!(
        "  Buffer size: {} samples ({:.1}ms latency)",
        config.ring_buffer_size,
        config.latency_ms()
    );
    println!("  Total samples: {}\n", player_info.total_samples);

    // Start streaming
    let playback_start = Instant::now();
    let context = StreamingContext::start(player_info.player, config, player_info.color_filter)?;

    // Run visualization loop
    run_visualization_loop(&context);

    // Shutdown and display statistics
    let total_time = playback_start.elapsed();
    let final_stats = context.streamer.get_stats();
    context.shutdown();
    println!("\n=== Playback Statistics ===");
    println!("Duration:          {:.2} seconds", total_time.as_secs_f32());
    println!("Samples played:    {}", final_stats.samples_played);
    println!("Overrun events:    {}", final_stats.overrun_count);
    println!("Buffer latency:    {:.1} ms", config.latency_ms());
    println!(
        "Memory used:       {} bytes (ring buffer)",
        config.ring_buffer_size * std::mem::size_of::<f32>()
    );
    println!("\nPlayback complete!");

    Ok(())
}
