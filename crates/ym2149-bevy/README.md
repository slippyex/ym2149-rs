# ym2149-bevy

A [Bevy](https://bevyengine.org/) plugin for real-time YM2149 PSG audio playback and visualization. This crate integrates the high-fidelity [ym2149](https://github.com/markusvelten/ym2149-rs) emulator with Bevy's ECS architecture, providing seamless YM file playback with live channel visualization.

## Features

- **Real-time YM2149 Audio Playback**: Stream YM2-YM6 format files with cycle-accurate emulation
- **Flexible Playback Control**: Play, pause, restart, volume adjustment, and loop support
- **Live Channel Visualization**: Real-time visual feedback for all three PSG channels
- **Metadata Display**: Automatic extraction and display of song title and artist information
- **Frame-by-Frame Control**: Access to individual playback frames for custom effects or analysis
- **Time-Accurate Pacing**: Proper time-based frame advancement matching the original YM file playback rate
- **Audio Buffering**: Configurable ring buffer for smooth, artifact-free playback
- **Modular Architecture**: Clean separation between audio, visualization, and playback systems

## Quick Start

### Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
bevy = "0.15"
ym2149-bevy = "0.1"
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

## API Overview

### Core Components

#### `Ym2149Playback`

The main component for controlling YM file playback. Spawn this component on an entity to start playing a YM file.

```rust
// Create and spawn a playback entity
let playback = Ym2149Playback::new("examples/song.ym");
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

// Interactive channel visualization bars
create_channel_visualization(&mut commands, 3);  // 3 channels for YM2149
```

#### Custom Visualization

All visualization components are exported for custom UI implementations:

- `PlaybackStatusDisplay` - Component for status text
- `SongInfoDisplay` - Component for song metadata
- `DetailedChannelDisplay` - Component for channel details
- `ChannelVisualization` - Component tracking channel output levels
- `ChannelBar` - Component for animated bar fills

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

The plugin uses a trait-based [`AudioSink`](https://docs.rs/ym2149-bevy/latest/ym2149_bevy/trait.AudioSink.html) abstraction for audio output, allowing you to bring your own implementation while rodio remains the default.

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

### Accessing Channel Data

The `ChannelVisualization` component tracks real-time output levels:

```rust
fn analyze_channels(
    visualizations: Query<&ChannelVisualization>,
) {
    for viz in visualizations.iter() {
        println!(
            "Channel {}: {:.2}",
            viz.channel_index,
            viz.output_level
        );
    }
}
```

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
cargo build -p ym2149-bevy

# Run the example
cargo run --example basic_playback -p ym2149-bevy

# Run tests
cargo test -p ym2149-bevy

# Generate documentation
cargo doc -p ym2149-bevy --open
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

- [ym2149](https://github.com/markusvelten/ym2149-rs) - Core emulator library
- [Bevy](https://bevyengine.org/) - Game engine documentation
- [rodio](https://github.com/RustAudio/rodio) - Audio output backend
