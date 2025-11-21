//! Bevy audio plugin for YM2149 PSG emulator
//!
//! This crate provides a Bevy plugin for playing YM2149 audio files with real-time visualization
//! using the high-fidelity [ym2149](https://crates.io/crates/ym2149) emulator library.
//!
//! The plugin handles all aspects of YM file playback through Bevy's ECS architecture:
//! - File loading and metadata extraction
//! - Time-accurate frame advancement and audio generation
//! - Real-time visualization of channel activity
//! - Flexible playback control (play, pause, restart, volume)
//!
//! # Features
//!
//! - **Real-time YM2149 Audio Playback**: Stream YM2-YM6 format files with cycle-accurate emulation
//! - **Flexible Playback Control**: Play, pause, restart, volume adjustment, and loop support
//! - **Live Channel Visualization**: Real-time visual feedback for all three PSG channels with frequency/note info
//! - **Metadata Display**: Automatic extraction and display of song title and artist information
//! - **Frame-by-Frame Access**: Direct access to individual playback frames for analysis
//! - **Time-Accurate Pacing**: Proper time-based frame advancement matching original YM file rate
//! - **Audio Buffering**: Ring buffer architecture for smooth, artifact-free playback
//! - **Multiple Playbacks**: Support for simultaneous independent YM file playbacks
//!
//! # Quick Start
//!
//! ```no_run
//! use bevy::prelude::*;
//! use bevy_ym2149::{Ym2149Plugin, Ym2149Playback};
//!
//! fn main() {
//!     App::new()
//!         .add_plugins(DefaultPlugins)
//!         .add_plugins(Ym2149Plugin::default())
//!         .add_systems(Startup, setup)
//!         .run();
//! }
//!
//! fn setup(mut commands: Commands) {
//!     commands.spawn(Camera2d::default());
//!     commands.spawn(Ym2149Playback::new("path/to/song.ym"));
//! }
//! ```
//!
//! # Playback Control
//!
//! Control playback through the [`Ym2149Playback`] component:
//!
//! ```no_run
//! use bevy::prelude::*;
//! use bevy_ym2149::Ym2149Playback;
//!
//! fn playback_control(
//!     mut playbacks: Query<&mut Ym2149Playback>,
//!     keyboard: Res<ButtonInput<KeyCode>>,
//! ) {
//!     for mut playback in playbacks.iter_mut() {
//!         if keyboard.just_pressed(KeyCode::Space) {
//!             if playback.is_playing() {
//!                 playback.pause();
//!             } else {
//!                 playback.play();
//!             }
//!         }
//!         if keyboard.just_pressed(KeyCode::ArrowUp) {
//!             let new_volume = (playback.volume + 0.1).min(1.0);
//!             playback.set_volume(new_volume);
//!         }
//!     }
//! }
//! ```
//!
//! # Visualization
//!
//! Visualization widgets now live in the companion crate `bevy_ym2149_viz`. Add
//! [`Ym2149VizPlugin`](https://docs.rs/bevy_ym2149_viz) alongside this crate's
//! [`Ym2149Plugin`] and use the helpers provided there to build UI components.
//!
//! # Architecture
//!
//! The plugin uses Bevy's ECS with three main systems:
//!
//! 1. **Initialization** - Loads YM files and creates emulator instances
//! 2. **Playback** - Advances frames using time-based pacing and generates audio
//! 3. **Visualization** - Updates UI with current playback state and channel information
//!
//! Audio flows through a thread-safe ring buffer from the emulator to rodio's audio device.
//!
//! # Module Organization
//!
//! - [`playback`] - Core playback component and state management
//! - [`plugin`] - Bevy plugin integration and systems
//! - [`audio_source`] - YM file loading and Bevy audio integration via Decodable
//! - [`bevy_ym2149_viz`](https://crates.io/crates/bevy_ym2149_viz) - Optional UI components and display helpers

pub mod audio_bridge;
pub mod audio_reactive;
pub mod audio_source;
pub mod diagnostics;
pub mod error;
pub mod events;
pub mod music_state;
pub mod oscilloscope;
pub mod patterns;
pub mod playback;
pub mod playlist;
pub mod plugin;
pub mod presets;
pub mod song_player;
pub mod synth;

pub use ::ym2149::*;
pub use audio_bridge::{
    AudioBridgeBuffers, AudioBridgeMix, AudioBridgeMixes, AudioBridgeTargets, BridgeAudioDevice,
    BridgeAudioSinks,
};
pub use audio_reactive::{AudioReactiveState, ReactiveMetrics};
pub use audio_source::{Ym2149AudioSource, Ym2149Loader, Ym2149Metadata};
pub use diagnostics::{BUFFER_FILL_PATH, FRAME_POSITION_PATH, update_diagnostics};
pub use error::{BevyYm2149Error, Result};
pub use events::{
    AudioBridgeRequest, ChannelSnapshot, MusicStateRequest, PatternTriggered, PlaybackFrameMarker,
    PlaylistAdvanceRequest, TrackFinished, TrackStarted, YmSfxRequest,
};
pub use music_state::{MusicStateDefinition, MusicStateGraph, process_music_state_requests};
pub use oscilloscope::OscilloscopeBuffer;
pub use patterns::{PatternTrigger, PatternTriggerSet};
pub use playback::{PlaybackState, Ym2149Playback, Ym2149Settings};
pub use playlist::{
    CrossfadeConfig, CrossfadeTrigger, CrossfadeWindow, PlaylistMode, PlaylistSource,
    Ym2149Playlist, Ym2149PlaylistLoader, Ym2149PlaylistPlayer, advance_playlist_players,
    drive_crossfade_playlists, handle_playlist_requests, register_playlist_assets,
};
pub use plugin::{Ym2149Plugin, Ym2149PluginConfig};
pub use synth::YmSynthController;
