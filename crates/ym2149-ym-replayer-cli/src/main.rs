//! YM2149 PSG Real-time Streaming Playback CLI
//!
//! Command-line player for YM chiptune files featuring:
//! - Real-time audio streaming with low latency
//! - Terminal-based visualization
//! - Interactive playback control
//! - YM2149 hardware emulation

mod args;
mod player_factory;
mod streaming;
mod visualization;

use std::time::Instant;
use ym2149::Ym2149Backend;
use ym2149::streaming::{DEFAULT_SAMPLE_RATE, StreamConfig};
use ym2149_arkos_replayer::ArkosPlayer;
use ym2149_ay_replayer::{AyPlaybackState, AyPlayer};
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
}

impl ArkosPlayerWrapper {
    pub fn new(player: ArkosPlayer) -> Self {
        Self { player }
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
        // For Arkos, create a basic snapshot with zeros
        // TODO: Extract actual PSG state from PsgBank
        VisualSnapshot {
            registers: [0u8; 16],
            sync_buzzer: false,
            sid_active: [false; 3],
            drum_active: [false; 3],
        }
    }

    fn set_channel_mute(&mut self, _channel: usize, _mute: bool) {
        // TODO: Implement channel muting in PsgBank
    }

    fn is_channel_muted(&self, _channel: usize) -> bool {
        false
    }

    fn get_playback_position(&self) -> f32 {
        // TODO: Calculate based on current_position / end_position
        0.0
    }

    fn set_color_filter(&mut self, _enabled: bool) {
        // Not applicable for Arkos
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
        self.player
            .play()
            .map_err(|e| format!("AY play error: {e}").into())
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
        match self.player.state() {
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
    let context = StreamingContext::start(player_info.player, config)?;

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
