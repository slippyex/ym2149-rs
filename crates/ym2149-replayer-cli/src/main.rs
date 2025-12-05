//! YM2149 PSG Real-time Streaming Playback CLI
//!
//! Command-line player for YM chiptune files featuring:
//! - Real-time audio streaming with low latency
//! - Terminal-based visualization
//! - Interactive playback control
//! - YM2149 hardware emulation
//! - Directory playback with playlist selection

mod args;
mod audio;
mod player_factory;
mod playlist;
mod streaming;
mod tui;
mod visualization;
mod viz_helpers;

use audio::{DEFAULT_SAMPLE_RATE, StreamConfig};
use parking_lot::Mutex;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use ym2149::Ym2149Backend;
use ym2149_arkos_replayer::ArkosPlayer;
use ym2149_ay_replayer::{AyPlayer, CPC_UNSUPPORTED_MSG};
use ym2149_common::ChiptunePlayerBase;
use ym2149_sndh_replayer::SndhPlayer;
use ym2149_ym_replayer::player::ym_player::YmPlayerGeneric;

use args::CliArgs;
use player_factory::{create_demo_player, create_player};
use playlist::Playlist;
use streaming::StreamingContext;
use tui::{CaptureBuffer, SongMetadata, run_tui_loop_with_playlist, terminal_supports_tui};
use visualization::run_visualization_loop;

/// Maximum number of PSG chips supported for visualization.
pub const MAX_PSG_COUNT: usize = 4;

/// Snapshot of chip state for visualization.
#[derive(Clone, Copy)]
pub struct VisualSnapshot {
    /// YM2149 register values for each PSG chip (R0-R15 per chip)
    pub registers: [[u8; 16]; MAX_PSG_COUNT],
    /// Number of active PSG chips
    pub psg_count: usize,
    /// Sync buzzer effect active (from YM metadata, not detected)
    pub sync_buzzer: bool,
    /// SID voice effects active per channel (detected from registers or metadata)
    pub sid_active: [bool; MAX_PSG_COUNT * 3],
    /// Drum effects active per channel (detected from registers or metadata)
    pub drum_active: [bool; MAX_PSG_COUNT * 3],
}

impl VisualSnapshot {
    /// Detect SID/drum effects from register patterns.
    ///
    /// This heuristic detects effects when metadata is not available
    /// (SNDH/AY formats). It analyzes envelope usage and amplitude patterns.
    pub fn detect_effects_from_registers(&mut self) {
        for psg_idx in 0..self.psg_count {
            let regs = &self.registers[psg_idx];
            let base_ch = psg_idx * 3;

            // Get envelope info
            let env_period = (regs[11] as u16) | ((regs[12] as u16) << 8);
            let env_shape = regs[13] & 0x0F;

            // SID detection: envelope enabled with active/complex shape
            // Shapes 8-15 are "complex" (sustaining/cycling)
            let is_complex_env = env_shape >= 8;
            let env_active = env_period > 0;

            for local_ch in 0..3 {
                let global_ch = base_ch + local_ch;
                let amp_reg = regs[8 + local_ch];
                let env_enabled = (amp_reg & 0x10) != 0;
                let amplitude = amp_reg & 0x0F;

                // Skip if already set (from YM metadata)
                if !self.sid_active[global_ch] {
                    // SID: envelope mode with complex shape and reasonable period
                    // Real SID effects typically use fast envelope periods (< 1000)
                    self.sid_active[global_ch] =
                        env_enabled && is_complex_env && env_active && env_period < 2000;
                }

                if !self.drum_active[global_ch] {
                    // DRUM: high amplitude spikes without envelope
                    // DigiDrum typically uses direct amplitude writes (no envelope)
                    // with values near max (13-15)
                    self.drum_active[global_ch] = !env_enabled && amplitude >= 13;
                }
            }
        }
    }
}

/// CLI-specific trait for real-time chip emulation with visualization.
///
/// This trait extends [`ChiptunePlayerBase`] with CLI-specific methods for
/// visualization and effects. All playback methods are inherited from the base trait.
pub trait RealtimeChip: ChiptunePlayerBase {
    /// Get current chip state for visualization.
    fn visual_snapshot(&self) -> VisualSnapshot;

    /// Enable/disable ST color filter.
    fn set_color_filter(&mut self, enabled: bool);

    /// Optional reason why playback can't continue.
    fn unsupported_reason(&self) -> Option<&'static str> {
        None
    }
}

impl<B: Ym2149Backend + 'static> RealtimeChip for YmPlayerGeneric<B> {
    fn visual_snapshot(&self) -> VisualSnapshot {
        let regs = self.dump_registers();
        let (sync, sid, drum) = self.get_active_effects();
        let mut registers = [[0u8; 16]; MAX_PSG_COUNT];
        registers[0] = regs;
        let mut sid_active = [false; MAX_PSG_COUNT * 3];
        let mut drum_active = [false; MAX_PSG_COUNT * 3];
        sid_active[..3].copy_from_slice(&sid);
        drum_active[..3].copy_from_slice(&drum);
        VisualSnapshot {
            registers,
            psg_count: 1,
            sync_buzzer: sync,
            sid_active,
            drum_active,
        }
    }

    fn set_color_filter(&mut self, enabled: bool) {
        YmPlayerGeneric::set_color_filter(self, enabled);
    }
}

/// Macro to implement ChiptunePlayerBase by delegating to an inner player field.
macro_rules! delegate_chiptune_player_base {
    ($wrapper:ty, $field:ident) => {
        impl ChiptunePlayerBase for $wrapper {
            fn play(&mut self) {
                ChiptunePlayerBase::play(&mut self.$field);
            }
            fn pause(&mut self) {
                ChiptunePlayerBase::pause(&mut self.$field);
            }
            fn stop(&mut self) {
                ChiptunePlayerBase::stop(&mut self.$field);
            }
            fn state(&self) -> ym2149_common::PlaybackState {
                ChiptunePlayerBase::state(&self.$field)
            }
            fn generate_samples_into(&mut self, buffer: &mut [f32]) {
                ChiptunePlayerBase::generate_samples_into(&mut self.$field, buffer);
            }
            fn set_channel_mute(&mut self, channel: usize, mute: bool) {
                ChiptunePlayerBase::set_channel_mute(&mut self.$field, channel, mute);
            }
            fn is_channel_muted(&self, channel: usize) -> bool {
                ChiptunePlayerBase::is_channel_muted(&self.$field, channel)
            }
            fn playback_position(&self) -> f32 {
                ChiptunePlayerBase::playback_position(&self.$field)
            }
            fn subsong_count(&self) -> usize {
                ChiptunePlayerBase::subsong_count(&self.$field)
            }
            fn current_subsong(&self) -> usize {
                ChiptunePlayerBase::current_subsong(&self.$field)
            }
            fn set_subsong(&mut self, index: usize) -> bool {
                ChiptunePlayerBase::set_subsong(&mut self.$field, index)
            }
            fn psg_count(&self) -> usize {
                ChiptunePlayerBase::psg_count(&self.$field)
            }
        }
    };
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

delegate_chiptune_player_base!(ArkosPlayerWrapper, player);

impl RealtimeChip for ArkosPlayerWrapper {
    fn visual_snapshot(&self) -> VisualSnapshot {
        let psg_count = self.player.psg_count().min(MAX_PSG_COUNT);
        let mut registers = [[0u8; 16]; MAX_PSG_COUNT];
        for (i, reg) in registers.iter_mut().enumerate().take(psg_count) {
            if let Some(chip) = self.player.chip(i) {
                *reg = chip.dump_registers();
            }
        }
        VisualSnapshot {
            registers,
            psg_count,
            sync_buzzer: false,
            sid_active: [false; MAX_PSG_COUNT * 3],
            drum_active: [false; MAX_PSG_COUNT * 3],
        }
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

delegate_chiptune_player_base!(AyPlayerWrapper, player);

impl RealtimeChip for AyPlayerWrapper {
    fn visual_snapshot(&self) -> VisualSnapshot {
        let mut registers = [[0u8; 16]; MAX_PSG_COUNT];
        registers[0] = self.player.chip().dump_registers();
        VisualSnapshot {
            registers,
            psg_count: 1,
            sync_buzzer: false,
            sid_active: [false; MAX_PSG_COUNT * 3],
            drum_active: [false; MAX_PSG_COUNT * 3],
        }
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
        ChiptunePlayer::metadata(&self.player)
    }
}

delegate_chiptune_player_base!(SndhPlayerWrapper, player);

impl RealtimeChip for SndhPlayerWrapper {
    fn visual_snapshot(&self) -> VisualSnapshot {
        // SNDH uses native 68000 code - extract YM registers from the emulated PSG
        let mut registers = [[0u8; 16]; MAX_PSG_COUNT];
        registers[0] = self.player.ym2149().dump_registers();
        VisualSnapshot {
            registers,
            psg_count: 1,
            sync_buzzer: false,
            sid_active: [false; MAX_PSG_COUNT * 3],
            drum_active: [false; MAX_PSG_COUNT * 3],
        }
    }

    fn set_color_filter(&mut self, _enabled: bool) {
        // Not applicable for SNDH (uses actual 68000 code)
    }
}

fn main() -> ym2149_ym_replayer::Result<()> {
    // Parse command-line arguments
    let args = CliArgs::parse();

    // Check if we'll use TUI mode upfront (to suppress unnecessary output)
    let will_use_tui = terminal_supports_tui();

    if !will_use_tui {
        println!("YM2149 PSG Emulator - Real-time Streaming Playback");
        println!("===================================================\n");
    }

    if args.show_help {
        CliArgs::print_help();
        return if args.file_path.is_none() {
            Ok(())
        } else {
            Err("Invalid arguments".into())
        };
    }

    // Check if input is a directory
    let is_directory = args
        .file_path
        .as_ref()
        .map(|p| Path::new(p).is_dir())
        .unwrap_or(false);

    // Load playlist if directory mode
    let playlist = if is_directory {
        let path = Path::new(args.file_path.as_ref().unwrap());
        if !will_use_tui {
            println!("Scanning directory: {}\n", path.display());
        }
        match Playlist::scan_directory(path) {
            Ok(pl) if !pl.is_empty() => {
                if !will_use_tui {
                    println!("Found {} songs\n", pl.len());
                }
                Some(pl)
            }
            Ok(_) => {
                return Err("No supported music files found in directory".into());
            }
            Err(e) => {
                return Err(format!("Failed to scan directory: {e}").into());
            }
        }
    } else {
        None
    };

    // Determine initial file to play
    let initial_file = if let Some(ref pl) = playlist {
        // Start with first song in playlist
        pl.entries
            .first()
            .map(|e| e.path.to_string_lossy().to_string())
    } else {
        args.file_path.clone()
    };

    // Create player instance
    let player_info = match initial_file {
        Some(ref file_path) => {
            create_player(file_path, args.chip_choice, args.color_filter_override)?
        }
        None => create_demo_player(args.chip_choice)?,
    };

    // Display file information (only in non-TUI mode)
    if !will_use_tui {
        println!("File Information:");
        println!("{}\n", player_info.song_info);
        println!("Selected Chip: {}\n", args.chip_choice);
    }

    // Configure streaming
    let config = StreamConfig::low_latency(DEFAULT_SAMPLE_RATE);
    if !will_use_tui {
        println!("Streaming Configuration:");
        println!("  Sample rate: {} Hz", config.sample_rate);
        println!(
            "  Buffer size: {} samples ({:.1}ms latency)",
            config.ring_buffer_size,
            config.latency_ms()
        );
        println!("  Total samples: {}\n", player_info.total_samples);
    }

    // Use TUI mode (already determined above)
    let use_tui = will_use_tui;

    // Extract metadata before moving player_info
    let song_metadata = SongMetadata {
        title: player_info.title.clone(),
        author: player_info.author.clone(),
        format: player_info.format.clone(),
        duration_secs: player_info.total_samples as f32 / DEFAULT_SAMPLE_RATE as f32,
    };

    // Start streaming (with capture buffer if using TUI)
    // In playlist mode, start paused so user can select a song first
    let playback_start = Instant::now();
    let context = if use_tui {
        let capture = Arc::new(Mutex::new(CaptureBuffer::new()));
        if is_directory {
            // Playlist mode: start paused, user selects song first
            StreamingContext::start_paused(
                player_info.player,
                config,
                player_info.color_filter,
                Some(capture),
            )?
        } else {
            // Single file mode: start playing immediately
            StreamingContext::start_with_capture(
                player_info.player,
                config,
                player_info.color_filter,
                Some(capture),
            )?
        }
    } else {
        StreamingContext::start(player_info.player, config, player_info.color_filter)?
    };

    // Create player loader closure for song switching
    let chip_choice = args.chip_choice;
    let color_filter_override = args.color_filter_override;
    let player_loader: Option<tui::PlayerLoader> = if is_directory {
        Some(Box::new(move |path: &std::path::Path| {
            let path_str = path.to_string_lossy().to_string();
            match create_player(&path_str, chip_choice, color_filter_override) {
                Ok(info) => Some((
                    info.player,
                    SongMetadata {
                        title: info.title,
                        author: info.author,
                        format: info.format,
                        duration_secs: info.total_samples as f32 / DEFAULT_SAMPLE_RATE as f32,
                    },
                )),
                Err(e) => {
                    eprintln!("Failed to load song: {e}");
                    None
                }
            }
        }))
    } else {
        None
    };

    // Run visualization loop (TUI or classic)
    if use_tui
        && let Some(ref capture) = context.capture
        && let Err(e) = run_tui_loop_with_playlist(
            &context,
            Arc::clone(capture),
            song_metadata,
            playlist,
            player_loader,
        )
    {
        eprintln!("TUI error: {e}");
    } else if !use_tui {
        run_visualization_loop(&context);
    }

    // Shutdown and display statistics
    let total_time = playback_start.elapsed();
    let final_stats = context.streamer.get_stats();
    context.shutdown();

    // Only print stats if not using TUI (TUI already shows them)
    if !use_tui {
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
    }

    Ok(())
}
