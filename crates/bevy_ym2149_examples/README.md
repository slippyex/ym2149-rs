# bevy_ym2149_examples

Example applications demonstrating the bevy_ym2149 plugin.

This crate contains comprehensive runnable examples showing how to use the YM2149 PSG emulator plugin in Bevy applications, from basic playback to advanced visualization and audio routing.

## Example Matrix

| Example | Highlights |
|---------|------------|
| `basic_example` | Minimal playback & keyboard controls |
| `crossfade_example` | Dual-deck playlist with seamless crossfades |
| `advanced_example` | Tracker UI, oscilloscope, drag-and-drop, audio bridge |
| `feature_showcase` | Multiple playbacks, playlists, music states, diagnostics |
| `demoscene` | Shader-heavy CRT pipeline + synchronized overlays |
| `playlist_crossfade_example` | Selectable playlist UI with crossfades |

## Examples

### basic_example
Minimal example showing:
- Creating a Bevy app with the YM2149 plugin
- Loading and playing a YM file
- Basic keyboard controls (Play/Pause, Restart, Volume)

**Run:** `cargo run --example basic_example -p bevy_ym2149_examples`

### crossfade_example
Focused demonstration of the playlist crossfade feature:
- Builds a two-track playlist from bundled assets
- Uses `CrossfadeConfig::start_at_seconds(15.0).with_window_seconds(15.0)` so a new deck fades in every 15s while the overlap window lasts 15s
- Loops indefinitely so you can hear the recurring blend cadence

**Run:** `cargo run --example crossfade_example -p bevy_ym2149_examples`

### playlist_crossfade_example
Selectable playlist UI with manual crossfades:
- Reads all bundled songs in `assets/music`, displays title/author plus filename
- Arrow Up/Down to select, `Enter` starts a 5s crossfade to the highlighted song, `Esc` quits
- Highlights the currently playing entry while fading so you always see the active deck

**Run:** `cargo run --example playlist_crossfade_example -p bevy_ym2149_examples`

### advanced_example
Advanced features including:
- Real-time visualization (oscilloscope, channel display, spectrum analysis)
- File drag-and-drop loading
- Audio bridge mixing with volume and pan controls
- Keyboard-based playback control

**Run:** `cargo run --example advanced_example -p bevy_ym2149_examples`

<img src="../../docs/screenshots/advanced_example.png" alt="Advanced example UI" width="780">

### feature_showcase
Comprehensive demonstration of:
- Multiple simultaneous YM file playbacks
- Playlist management with automatic progression
- Music state graphs for dynamic music transitions
- Audio bridge mixing with real-time parameter control
- Playback diagnostics and frame position tracking
- Event-driven architecture for track transitions

**Run:** `cargo run --example feature_showcase -p bevy_ym2149_examples`

### demoscene
Demoscene-style example featuring:
- YM2149 music playback synchronized with visuals
- Bitmap font rendering with multiple animated overlay modes (Typewriter, BounceIn, CascadeZoom, etc.)
- Responsive layout with swing/breathing motion and DPI-aware scaling
- Dual-pass WGSL pipeline: raymarched scene + CRT post-process shader (toggle with `C`)
- Fullscreen / Windowed mode switching (toggle with `F`)
- Easing functions for text animation timing and shader sequencing

**Run:** `cargo run --example demoscene -p bevy_ym2149_examples`

<img src="../../docs/screenshots/demoscene.png" alt="Demoscene example" width="780">

**Controls**
- `C` – Toggle CRT post-processing (single-pass scene vs. CRT overlay)
- `F` - Toggle fullscreen mode
- `Esc` – Exit the application


## Crate Structure

This crate provides example applications plus a shared utility library for asset configuration:

```
bevy_ym2149_examples/
├── src/
│   └── lib.rs                    # Shared utilities (ASSET_BASE, example_plugins helper)
├── examples/                     # Runnable example applications
│   ├── basic_example.rs
│   ├── crossfade_example.rs
│   ├── advanced_example.rs
│   ├── feature_showcase.rs
│   └── demoscene.rs
├── assets/                       # Shared assets for examples
│   ├── music/
│   ├── fonts/
│   └── shaders/
├── Cargo.toml
└── README.md
```

### Shared Library Module

The `src/lib.rs` module exports:
- **`ASSET_BASE`**: Compile-time constant pointing to the crate's assets directory
- **`example_plugins()`**: Helper function that configures DefaultPlugins with the correct asset path

All examples use these to ensure consistent asset loading from any directory.

## Asset Configuration

All examples use compile-time asset path resolution to ensure assets are found regardless of execution directory.

### How It Works

The `ASSET_BASE` constant is defined at compile time:
```rust
const ASSET_BASE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets");
```

This resolves to the examples crate's absolute path to the assets directory during compilation. Examples can be run from any directory:

```bash
# From workspace root
cargo run --example basic_example -p bevy_ym2149_examples

# From crate directory
cd crates/bevy_ym2149_examples
cargo run --example basic_example

# From anywhere else - both work correctly!
```

Asset paths in code are relative to `ASSET_BASE`:
- Music files: `"music/ND-Toxygene.ym"`
- Fonts: `"fonts/demoscene_font.png"`
- Shaders: `"shaders/oldschool.wgsl"`

### Asset Loading Pattern Explained

**Why Compile-Time Resolution?**

Bevy's default `AssetPlugin` loads assets relative to the binary's execution directory. This creates friction during development:

```rust
// Standard Bevy approach (runtime working directory)
.add_plugins(DefaultPlugins)
// Assets must be in ./assets/ relative to where you run the command
```

This works well for shipped applications but causes issues in monorepos:
- Different teams run commands from different directories
- CI/CD environments have different working directories
- Development workflows may vary

**Our Approach: Compile-Time Path Resolution**

By embedding the full path at compile time, examples work consistently:

```rust
// From src/lib.rs (shared by all examples)
pub const ASSET_BASE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets");

// Usage in examples:
.add_plugins(DefaultPlugins.set(AssetPlugin {
    file_path: ASSET_BASE.into(),
    ..default()
}))
```

### Using This Pattern in Your Project

**For examples and development tools** (recommended to use our approach):
```rust
use bevy_ym2149_examples::example_plugins;

fn main() {
    App::new()
        .add_plugins(example_plugins())
        // rest of code
        .run();
}
```

**For production applications** (use Bevy's standard pattern):
```rust
fn main() {
    App::new()
        .add_plugins(DefaultPlugins)  // Use default asset loading
        // rest of code
        .run();
}
```

Then ensure your assets directory is in the correct location relative to your binary when shipping.

## Asset Structure

The examples use the following asset structure:

```
assets/                          # Base directory set at compile time
├── music/                       # YM2149 music files
│   ├── Ashtray.ym
│   ├── Credits.ym
│   ├── ND-Toxygene.ym
│   ├── Prelude.ym
│   ├── Scout.ym
│   └── Steps.ym
├── fonts/                       # Bitmap fonts for UI
│   └── demoscene_font.png
└── shaders/                     # Custom WGSL shaders
    ├── crt_post.wgsl
    └── oldschool.wgsl
```

## Building and Running

To build all examples:
```bash
cargo build --examples -p bevy_ym2149_examples
```

To run a specific example:
```bash
cargo run --example <example_name> -p bevy_ym2149_examples
```

## Features

This crate does not define Cargo feature flags. Visualization-heavy demos simply register `Ym2149VizPlugin` in code, while headless-friendly ones omit it. Run the examples directly with the commands shown above—no `--no-default-features` juggling required. (The `advanced_example` always enables the viz plugin, so keep the `bevy_ym2149_viz` dependency if you plan to run it.)

## Dependencies

The examples depend on:
- `bevy_ym2149` - The YM2149 plugin for Bevy
- `bevy` - The Bevy game engine (0.17)
- `bevy_mesh` - Mesh rendering for Bevy
- `ym2149` - YM2149 core emulator

## Notes

- Each example demonstrates different aspects of the plugin
- Examples are designed to be self-contained and runnable independently
- Some examples use visualization features that can be toggled
- The demoscene example showcases advanced shader integration

## License

MIT - See the main repository for license information.
