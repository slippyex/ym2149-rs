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
//! use ym2149_bevy::{Ym2149Plugin, Ym2149Playback};
//!
//! fn main() {
//!     App::new()
//!         .add_plugins(DefaultPlugins)
//!         .add_plugins(Ym2149Plugin)
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
//! use ym2149_bevy::Ym2149Playback;
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
//! The plugin provides helper functions to create UI displays:
//!
//! ```no_run
//! use bevy::prelude::*;
//! use ym2149_bevy::{create_status_display, create_detailed_channel_display, create_channel_visualization};
//!
//! fn setup_ui(mut commands: Commands) {
//!     // Top panel with song info and playback status
//!     create_status_display(&mut commands);
//!
//!     // Detailed channel information (flags, frequency, notes)
//!     create_detailed_channel_display(&mut commands);
//!
//!     // Interactive visualization bars for 3 channels
//!     create_channel_visualization(&mut commands, 3);
//! }
//! ```
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
//! - [`audio_sink`] - Trait-based audio output abstraction (pluggable implementations)
//! - [`audio_source`] - YM file loading and metadata extraction
//! - [`visualization`] - UI components and display helpers

pub mod audio_sink;
pub mod audio_source;
pub mod playback;
pub mod plugin;
pub mod visualization;

pub use audio_sink::{AudioSink, BoxedAudioSink};
pub use audio_source::{Ym2149AudioSource, Ym2149Loader, Ym2149Metadata};
pub use playback::{PlaybackState, Ym2149Playback, Ym2149Settings};
pub use plugin::Ym2149Plugin;
pub use visualization::{
    create_channel_visualization, create_detailed_channel_display, create_song_info_display,
    create_status_display, update_channel_bars, ChannelBar, ChannelVisualization,
    DetailedChannelDisplay, PlaybackStatusDisplay, SongInfoDisplay,
};
