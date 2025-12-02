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
use std::time::Instant;
use ym2149::Ym2149Backend;
use ym2149_arkos_replayer::ArkosPlayer;
use ym2149_ay_replayer::{AyPlayer, CPC_UNSUPPORTED_MSG};
use ym2149_common::ChiptunePlayerBase;
use ym2149_sndh_replayer::SndhPlayer;
use ym2149_ym_replayer::player::ym_player::YmPlayerGeneric;

use args::CliArgs;
use player_factory::{create_demo_player, create_player};
use streaming::StreamingContext;
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
    /// Sync buzzer effect active
    pub sync_buzzer: bool,
    /// SID voice effects active per channel (up to 12 channels for 4 PSGs)
    pub sid_active: [bool; MAX_PSG_COUNT * 3],
    /// Drum effects active per channel (up to 12 channels for 4 PSGs)
    pub drum_active: [bool; MAX_PSG_COUNT * 3],
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
