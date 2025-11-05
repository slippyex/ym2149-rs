# bevy_ym2149

A [Bevy](https://bevyengine.org/) plugin for real-time YM2149 PSG audio playback and visualization. This crate integrates the high-fidelity [ym2149](https://github.com/markusvelten/ym2149-rs) emulator with Bevy's ECS architecture, providing seamless YM file playback with live channel visualization.

[![Crates.io](https://img.shields.io/crates/v/bevy_ym2149.svg)](https://crates.io/crates/bevy_ym2149)
[![Docs.rs](https://docs.rs/bevy_ym2149/badge.svg)](https://docs.rs/bevy_ym2149)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

## Features

- **Real-time YM2149 Audio Playback**: Stream YM2-YM6 format files with cycle-accurate emulation
- **Flexible Playback Control**: Play, pause, restart, volume adjustment, and loop support
- **Live Channel Visualization**: Real-time visual feedback for all three PSG channels
- **Metadata Display**: Automatic extraction and display of song title and artist information
- **Frame-by-Frame Control**: Access to individual playback frames for custom effects or analysis
- **Time-Accurate Pacing**: Proper time-based frame advancement matching the original YM file playback rate
- **Audio Buffering**: Configurable ring buffer for smooth, artifact-free playback
- **Modular Architecture**: Clean separation between audio, visualization, and playback systems
- **Playlist Playback**: Drive multiple tracks from `.ymplaylist` assets with automatic advancement
- **Channel Events**: Subscribe to per-frame channel snapshots and playback lifecycle messages
- **Spatial Audio**: Optional stereo panning that reacts to Bevy transforms
- **Music State Graph**: Named state machine for swapping playlists or sources at runtime
- **Shader Uniforms**: Live oscilloscope and spectrum data for custom rendering pipelines
- **Diagnostics Hooks**: Report buffer fill and frame position through Bevy's diagnostics system
- **Audio Bridge Buffers**: Mirror generated samples into Bevy's audio graph via `AudioBridgeBuffers`
- **Bridge Mixing Controls**: Adjust per-entity gain/pan for bridged audio via `AudioBridgeMixes`

## Quick Start

### Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
bevy = "0.15"
bevy_ym2149 = "0.5"
```

### Minimal Example

```rust
use bevy::prelude::*;
use ym2149_bevy::{Ym2149Plugin, Ym2149Playback};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(Ym2149Plugin)
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d::default());
    commands.spawn(Ym2149Playback::new("path/to/song.ym"));
}
```

### Plugin Configuration

`Ym2149Plugin` exposes feature toggles through [`Ym2149PluginConfig`](https://docs.rs/bevy_ym2149/latest/bevy_ym2149/struct.Ym2149PluginConfig.html). Each toggle maps directly to a subsystem (visualization, playlists, channel events, spatial audio, music state graph, shader uniforms, diagnostics, and audio bridge buffers):

```rust
use bevy::prelude::*;
use bevy_ym2149::{Ym2149Plugin, Ym2149PluginConfig};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(Ym2149Plugin::with_config(
            Ym2149PluginConfig::default()
                .playlists(true)
                .channel_events(true)
                .spatial_audio(true)
                .diagnostics(true)
                .bevy_audio_bridge(true),
        ))
        .add_systems(Startup, setup)
        .run();
}
```

Disable subsystems you don't need to keep the runtime lean (for example, turning off `visualization` for headless audio playback or skipping `spatial_audio` when you only require mono output).

## API Overview

### Core Components

#### `Ym2149Playback`

The main component for controlling YM file playback. Spawn this component on an entity to start playing a YM file.

```rust
// Create and spawn a playback entity
let playback = Ym2149Playback::new("path/to/your_song.ym");
commands.spawn(playback);

// Query and control playback
let mut playback = playbacks.iter_mut().next().unwrap();
playback.play();
playback.set_volume(0.8);
playback.pause();
playback.restart();
```

**Key Methods:**
- `new(path: &str)` - Create a new playback entity from a YM file
- `play()` - Start playback
- `pause()` - Pause playback
- `stop()` - Stop playback
- `restart()` - Restart from the beginning (reloads file from disk)
- `set_volume(volume: f32)` - Set volume (0.0 to 1.0)

**Key Fields:**
- `state: PlaybackState` - Current playback state (Idle, Playing, Paused, Finished)
- `frame_position: u32` - Current frame number
- `volume: f32` - Current volume (0.0 to 1.0)
- `song_title: String` - Extracted song title
- `song_author: String` - Extracted author name

#### `Ym2149Settings`

Global plugin settings accessible as a Bevy resource.

```rust
fn control_system(mut settings: ResMut<Ym2149Settings>) {
    settings.loop_enabled = true;  // Enable looping
}
```

**Available Settings:**
- `loop_enabled: bool` - Whether to loop the song after finishing

### Visualization Components

#### Built-in Display Functions

The plugin provides helper functions to create pre-configured UI displays:

```rust
// Top panel with song info and status
create_status_display(&mut commands);

// Detailed channel information (T/N/A/E flags, frequency, notes)
create_detailed_channel_display(&mut commands);

// Transport progress, loop indicator, and per-channel spectrum rows
create_channel_visualization(&mut commands, 3);  // 3 channels for YM2149
```

#### Custom Visualization

All visualization components are exported for custom UI implementations:

- `PlaybackStatusDisplay` - Component for status text
- `SongInfoDisplay` - Component for song metadata
- `DetailedChannelDisplay` - Component for channel details
- `SongProgressFill` / `SongProgressLabel` - Components for transport progress display
- `LoopStatusLabel` - Component reflecting the loop toggle
- `ChannelNoteLabel` / `ChannelFreqLabel` - Channel headers showing the active note and frequency
- `SpectrumBar` - Individual bar segments in the channel spectrum ribbon
- `Oscilloscope`, `OscilloscopePoint`, `OscilloscopeHead`, `OscilloscopeGridLine`, `ChannelBadge` - Building blocks used by the oscilloscope helpers
- `SpectrumBar` - Individual frequency bin visual in the spectrum rows
- `Oscilloscope`, `OscilloscopePoint`, `OscilloscopeHead` - Components that power the scope canvas
- `OscilloscopeGridLine`, `ChannelBadge` - Decorative helpers used by the built-in scope

```rust
// Create a custom status display
commands.spawn((
    Text::new("Custom Status"),
    Node { /* your layout */ },
    PlaybackStatusDisplay,
));
```

### Playback States

```rust
pub enum PlaybackState {
    Idle,      // Not playing
    Playing,   // Currently playing
    Paused,    // Paused
    Finished,  // Song finished
}
```

## Pluggable Audio Output

The plugin uses a trait-based [`AudioSink`](https://docs.rs/bevy_ym2149/latest/ym2149_bevy/trait.AudioSink.html) abstraction for audio output, allowing you to bring your own implementation while rodio remains the default.

### Using the Default Rodio Implementation

By default, the plugin automatically uses `RodioAudioSink` for real-time audio playback. No additional setup is required.

### Implementing a Custom Audio Sink

You can implement the `AudioSink` trait for custom outputs like WAV file writing, network streaming, or other mechanisms:

```rust
use ym2149_bevy::AudioSink;
use std::sync::Arc;

struct MyWavSink {
    // Your state
}

impl AudioSink for MyWavSink {
    fn push_samples(&self, samples: Vec<f32>) -> Result<(), String> {
        // Write samples to WAV file or stream
        Ok(())
    }

    fn pause(&self) {
        // Optional: handle pause
    }

    fn resume(&self) {
        // Optional: handle resume
    }

    fn buffer_fill_level(&self) -> f32 {
        // Return 0.0 (empty) to 1.0 (full)
        // For file output, return 0.5
        0.5
    }
}
```

### Using a Custom Sink (Future Enhancement)

While the current implementation uses rodio by default, the architecture is designed to support injecting custom sinks. Future versions may provide a resource-based mechanism for registering custom implementations without modifying the plugin code.

## Architecture

### System Flow

1. **Initialization** (`initialize_playback` system)
   - Loads YM file from disk
   - Creates emulator instance
   - Extracts song metadata
   - Initializes audio sink (defaults to rodio, but pluggable)

2. **Playback** (`update_playback` system)
   - Advances frames based on elapsed time
   - Generates audio samples at correct rate
   - Maintains 50Hz playback synchronization
   - Handles state transitions

3. **Visualization** (`update_playback_status` system)
   - Updates display texts with current state
   - Calculates frequency and note information
   - Monitors channel output levels
   - Refreshes UI every frame

4. **Bar Animation** (`update_channel_bars` system)
   - Scales bar widths based on channel output
   - Provides real-time visual feedback

### Key Design Decisions

**Time-Based Frame Advancement**

YM files don't play at Bevy's frame rate (60 FPS). Instead, the plugin uses accumulated delta time to determine when to generate the next frame:

```rust
// Pseudo-code
time_since_last_frame += delta_time;
if time_since_last_frame >= frame_duration {
    player.generate_frame();
    time_since_last_frame -= frame_duration;
}
```

This ensures audio plays at the exact rate specified in the YM file, typically 50Hz.

**Audio Sink Abstraction**

Audio samples flow from the emulator to a pluggable audio sink (trait-based):

```
YM2149 Emulator → AudioSink trait → RodioAudioSink (or custom implementation)
                                      ↓
                              Ring Buffer → rodio AudioDevice → Speaker
```

The default `RodioAudioSink` uses a ring buffer (~250ms at 44.1kHz) to prevent underruns while maintaining low latency. Users can implement the `AudioSink` trait for alternative outputs.

**Metadata Extraction**

Song title and artist information is extracted from the YM file header during initialization using the emulator's `format_info()` method.

## Controls and File Loading (Example)

The included `basic_playback` example implements the following controls:

### Keyboard Controls
- **SPACE**: Play/Pause toggle
- **R**: Restart from beginning
- **L**: Toggle looping
- **UP**: Increase volume
- **DOWN**: Decrease volume

### Drag and Drop
**The example supports drag-and-drop file loading!** Simply drag any `.ym` file into the window to load and play it immediately. This works with all YM format variants (YM2-YM6) and automatically decompresses LHA-compressed files.

This makes it easy to test playback of any YM file in your collection without modifying the code.

## Advanced Usage

### Playlists

`Ym2149Playlist` assets drive multi-track playback. They support filesystem paths, preloaded Bevy assets, or embedded byte arrays:

```ron
(
    mode: "loop",
    tracks: [
        { type: "file", path: "music/title.ym" },
        { type: "asset", path: "music/game.ym" },
        { type: "bytes", data: [/* YM payload */] },
    ],
)
```

Attach a `Ym2149PlaylistPlayer` to your playback entity to advance automatically:

```rust
use bevy::prelude::*;
use bevy_ym2149::{Ym2149Playback, Ym2149Playlist, Ym2149PlaylistPlayer};

let playlist: Handle<Ym2149Playlist> = asset_server.load("music/demo.ymplaylist");
commands.spawn((
    Ym2149Playback::default(),
    Ym2149PlaylistPlayer::new(playlist),
));
```

Dispatch `PlaylistAdvanceRequest` messages when you need to jump to specific indices.

### Channel Events & Observability

Enabling the `channel_events` toggle surfaces rich introspection:

- `ChannelSnapshot` – per-frame amplitude and frequency for each PSG channel
- `TrackStarted` / `TrackFinished` – lifecycle messages for playlist and state-graph logic

```rust
fn dump_snapshots(mut snapshots: MessageReader<ChannelSnapshot>) {
    for snapshot in snapshots.read() {
        info!(
            "entity={:?} channel={} amp={:.2} freq={:?}",
            snapshot.entity,
            snapshot.channel,
            snapshot.amplitude,
            snapshot.frequency
        );
    }
}
```

### Music State Graph

The `MusicStateGraph` resource maps names to playlists, file paths, or raw YM buffers, letting you express higher-level soundtrack flows:

```rust
fn configure_states(mut graph: ResMut<MusicStateGraph>, playlist: Handle<Ym2149Playlist>) {
    graph.set_target(Entity::from_raw(1)); // Default playback entity
    graph.insert("title", MusicStateDefinition::Playlist(playlist));
    graph.insert("battle", MusicStateDefinition::SourcePath("music/battle.ym".into()));
}

fn switch_to_battle(mut writer: MessageWriter<MusicStateRequest>) {
    writer.write(MusicStateRequest { state: "battle".into(), target: None });
}
```

### Audio Bridge Buffers

With `bevy_audio_bridge` enabled, `AudioBridgeRequest` messages mark entities whose generated stereo samples should be mirrored into `AudioBridgeBuffers`. Downstream systems (including Bevy's own audio graph or third-party DSP pipelines) can read these buffers every frame:

```rust
use bevy::prelude::*;
use bevy_ym2149::audio_bridge::{
    AudioBridgeBuffers, AudioBridgeMixes, AudioBridgeRequest,
};

fn mirror_audio(mut requests: MessageWriter<AudioBridgeRequest>, playback: Entity) {
    requests.write(AudioBridgeRequest { entity: playback });
}

fn consume_bridge(
    mut mixes: ResMut<AudioBridgeMixes>,
    buffers: Res<AudioBridgeBuffers>,
) {
    mixes.set_pan(my_entity, -0.35);
    mixes.set_volume_db(my_entity, -2.0);

    if let Some(samples) = buffers.0.get(&my_entity) {
        // Push `samples` into an AudioSink or perform custom processing.
    }
}
```

### Diagnostics

The optional diagnostics subsystem registers two entries (`ym2149/buffer_fill` and `ym2149/frame_position`) with Bevy's diagnostics store. Consume them via `DiagnosticsStore` or your preferred telemetry sink to monitor stream health.

### Feature Flags

- `visualization` (default): enables the oscilloscope UI helpers and related systems. Disable this
  feature (`--no-default-features`) if you only need audio playback and want to trim Bevy UI
  dependencies. All visualization APIs (`create_oscilloscope`, `update_oscilloscope`, etc.) are only
  available when the feature is enabled.

### Custom Playback Control

```rust
fn custom_playback_system(
    mut playbacks: Query<&mut Ym2149Playback>,
    keyboard: Res<ButtonInput<KeyCode>>,
) {
    for mut playback in playbacks.iter_mut() {
        if keyboard.just_pressed(KeyCode::Space) {
            if playback.is_playing() {
                playback.pause();
            } else {
                playback.play();
            }
        }
    }
}
```

### Accessing Spectrum Data

`SpectrumBar` entities expose the computed bin magnitude through their layout. The height of
each node is proportional to the energy in that frequency window:

```rust
fn inspect_spectrum(bars: Query<(&SpectrumBar, &Node)>) {
    for (bar, node) in bars.iter() {
        if let Val::Px(height) = node.height {
            println!("Channel {} bin {} height {:.1} px", bar.channel, bar.bin, height);
        }
    }
}
```

### Access to `ym2149-core`

`bevy_ym2149` re-exports the entire [`ym2149`](https://crates.io/crates/ym2149) crate so you can
use the low-level chip types alongside the Bevy integration:

```rust
use bevy_ym2149::ym2149::Ym2149;

fn tweak_chip(chip: &mut Ym2149) {
    chip.set_color_filter(true);
}
```

### Example Separation

Two examples ship with the crate:

- `basic_playback` — visualization-heavy tracker layout with live mix controls.
- `feature_showcase` — headless-friendly demo covering playlists, music states,
  diagnostics, and bridge mixing.

The `basic_playback` example is built entirely on the public API surface:

```rust
use bevy::asset::AssetServer;
use bevy::prelude::*;
use bevy_ym2149::{
    create_status_display, Ym2149AudioSource, Ym2149Playback, Ym2149Plugin,
};

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    // File-system path
    commands.spawn(Ym2149Playback::new("music/song.ym"));

    // Bevy asset handle (hot-reload, virtual file systems, etc.)
    let handle: Handle<Ym2149AudioSource> = asset_server.load("music/other_song.ym");
    commands.spawn(Ym2149Playback::from_asset(handle));

    // Raw in-memory bytes (e.g. fetched from network or embedded in the binary)
    let embedded = include_bytes!("../assets/demo.ym");
    commands.spawn(Ym2149Playback::from_bytes(&embedded[..]));

    create_status_display(&mut commands);
}
```

All helper UI builders and systems live under the `visualization` module, while the core playback
loop resides in `plugin::systems`. This keeps the plugin lean and makes it easy to opt out of the
default visualization layer if you only need audio playback.

### Multiple Simultaneous Playbacks

Spawn multiple `Ym2149Playback` entities to play different songs:

```rust
commands.spawn(Ym2149Playback::new("song1.ym"));
commands.spawn(Ym2149Playback::new("song2.ym"));
```

Each playback is independent with its own emulator instance and audio output.

## Frequency and Note Calculation

The plugin calculates musical notes from YM register values:

1. **Register Values** → **Period** (14-bit value from registers)
2. **Period** → **Frequency** (Hz = 2MHz / (16 × period))
3. **Frequency** → **MIDI Note** (logarithmic scale, A4 = 440Hz)
4. **MIDI Note** → **Note Name** (C0 to G8)

Example output: "C4" (Middle C), "A#5", "E3"

## YM File Format Support

Supports all standard YM formats:
- YM2 (Mad Max format with digi-drums)
- YM3/YM3b (Standard format)
- YM4 (Stereo mixdown)
- YM5 (Effects support)
- YM6 (Digidrums and additional effects)

Compressed YM files (LHA) are automatically decompressed during loading.

## Performance Considerations

- **Frame Generation**: ~0.5-1ms per 50Hz frame on modern hardware
- **Ring Buffer**: 250ms default (44.1kHz stereo) prevents audio underruns
- **Visualization Updates**: Every Bevy frame (60 FPS) without blocking audio
- **Memory Usage**: ~1-2MB per concurrent playback instance

## Troubleshooting

### No Audio Output

1. Verify the YM file path is correct
2. Check that `Ym2149Plugin` is added to the app
3. Ensure rodio can access system audio (permissions, audio device present)
4. Check console output for initialization logs

### Crackling or Audio Artifacts

1. Increase the ring buffer size (in `audio_output.rs`)
2. Ensure adequate system resources
3. Try reducing other system load

### Playback Too Fast/Slow

This shouldn't happen due to time-based frame advancement. If it does:
1. Check system time accuracy
2. Verify YM file isn't corrupted
3. Review `frame_duration` calculation in `plugin.rs`

## Building and Testing

```bash
# Build the crate
cargo build -p bevy_ym2149

# Run the example
cargo run --example basic_playback -p bevy_ym2149

# Run tests
cargo test -p bevy_ym2149

# Generate documentation
cargo doc -p bevy_ym2149 --open
```

## Integration with ym2149

This plugin uses the [ym2149](https://crates.io/crates/ym2149) crate for core emulation. All audio processing, register handling, and effects support come from the parent crate's cycle-accurate implementation.

## License

MIT — see the main repository LICENSE file.

## Contributing

Contributions are welcome! Please ensure:
- Code follows Bevy style conventions
- All public APIs are documented
- Examples work correctly
- Tests pass

## See Also

- [ym2149](https://github.com/slippyex/ym2149-rs) - Core emulator library
- [Bevy](https://bevyengine.org/) - Game engine documentation
- [rodio](https://github.com/RustAudio/rodio) - Audio output backend
