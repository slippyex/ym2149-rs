# YM2149-RS Monorepo

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

| Crate | crates.io | docs.rs |
|-------|-----------|---------|
| `ym2149` | [![ym2149](https://img.shields.io/crates/v/ym2149.svg?label=ym2149)](https://crates.io/crates/ym2149) | [![ym2149 docs](https://docs.rs/ym2149/badge.svg)](https://docs.rs/ym2149) |
| `ym2149-ym-replayer` | [![ym2149-ym-replayer](https://img.shields.io/crates/v/ym2149-ym-replayer.svg?label=ym2149-ym-replayer)](https://crates.io/crates/ym2149-ym-replayer) | [![ym2149-ym-replayer docs](https://docs.rs/ym2149-ym-replayer/badge.svg)](https://docs.rs/ym2149-ym-replayer) |
| `ym2149-arkos-replayer` | [![ym2149-arkos-replayer](https://img.shields.io/crates/v/ym2149-arkos-replayer.svg?label=ym2149-arkos-replayer)](https://crates.io/crates/ym2149-arkos-replayer) | [![ym2149-arkos-replayer docs](https://docs.rs/ym2149-arkos-replayer/badge.svg)](https://docs.rs/ym2149-arkos-replayer) |
| `bevy_ym2149` | [![bevy_ym2149](https://img.shields.io/crates/v/bevy_ym2149.svg?label=bevy_ym2149)](https://crates.io/crates/bevy_ym2149) | [![bevy_ym2149 docs](https://docs.rs/bevy_ym2149/badge.svg)](https://docs.rs/bevy_ym2149) |
| `bevy_ym2149_viz` | [![bevy_ym2149_viz](https://img.shields.io/crates/v/bevy_ym2149_viz.svg?label=bevy_ym2149_viz)](https://crates.io/crates/bevy_ym2149_viz) | [![bevy_ym2149_viz docs](https://docs.rs/bevy_ym2149_viz/badge.svg)](https://docs.rs/bevy_ym2149_viz) |
| `ym2149-bevy` | [![ym2149-bevy](https://img.shields.io/crates/v/ym2149-bevy.svg?label=ym2149-bevy)](https://crates.io/crates/ym2149-bevy) | – |

> Cycle-accurate Yamaha YM2149 tooling for Rust — from raw PSG emulation and YM/YMT importers to Arkos Tracker playback, CLI/export pipelines, Bevy integrations, visualization stacks, and a one-click WASM demo.

| Quick Links | |
|-------------|---|
| ▶️ [Web Player](https://slippyex.github.io/ym2149-rs/) | Cycle-accurate YM/AKS demo in the browser |
| 🧱 [Architecture](ARCHITECTURE.md) | Layered breakdown of emulator, replayers, and integrations |
| 🧭 [Quick Start](#quick-start) | Code snippets for core, CLI, Bevy, and exports |
| 🆕 [Changelog](CHANGELOG.md) | Recent features and compatibility notes |

## At a Glance

| 🧠 Core Emulator | 🪕 Audio Pipelines | 🕹️ Game & Bevy |
|------------------|-------------------|----------------|
| Integer-accurate PSG, YM1–YM6 & tracker helpers | Streaming playback, WAV/MP3 export, playlist automation | Plug-and-play Bevy plugins with diagnostics, viz, playlists |
| 🌐 Browser Ready | 📦 Monorepo Cohesion | 🧪 Quality |
| WASM player (147 KB) with LHA support & drag-drop | Shared versioning, unified docs, cross-crate tests | 165+ tests, curated fixtures, demoscene examples |


## 🎵 Try it in Your Browser

**[► Launch Web Player](https://slippyex.github.io/ym2149-rs/)**

Experience authentic Atari ST chiptune music directly in your browser! The WebAssembly player features:
- ✨ Full YM2-YM6 format support with LHA decompression
- 🎮 Play/Pause/Stop controls with progress bar
- 🔊 Volume control and channel muting (A/B/C)
- 📊 Real-time metadata display
- 📦 Only 147KB WASM module
- 🎯 Cycle-accurate YM2149 emulation

<details>
<summary>📸 Web Player Screenshot</summary>

![Web Player](docs/screenshots/web-player.png)

*Retro CRT-style interface with drag & drop file loading*
</details>

## Workspace Packages

| Crate | Purpose | Crates.io | Docs |
|-------|---------|-----------|------|
| [`ym2149`](crates/ym2149-core) | Core YM2149 chip emulator (cycle-accurate) | [crates.io/crates/ym2149](https://crates.io/crates/ym2149) | [docs.rs/ym2149](https://docs.rs/ym2149) |
| [`ym2149-ym-replayer`](crates/ym2149-ym-replayer) | YM file parsing and music playback (YM1-YM6, YMT1/YMT2 tracker) | [crates.io/crates/ym2149-ym-replayer](https://crates.io/crates/ym2149-ym-replayer) | [docs.rs/ym2149-ym-replayer](https://docs.rs/ym2149-ym-replayer) |
| [`ym2149-ym-replayer-cli`](crates/ym2149-ym-replayer-cli) | Standalone CLI player with streaming and export | Unpublished (workspace) | – |
| [`ym2149-softsynth`](crates/ym2149-softsynth) | Experimental software synthesizer backend (proof-of-concept) | Unpublished (workspace) | [crates/ym2149-softsynth/README.md](crates/ym2149-softsynth/README.md) |
| [`ym2149-arkos-replayer`](crates/ym2149-arkos-replayer) | Arkos Tracker 2/3 (.aks) parser and native multi-PSG player (pure Rust) | [crates.io/crates/ym2149-arkos-replayer](https://crates.io/crates/ym2149-arkos-replayer) | [docs.rs/ym2149-arkos-replayer](https://docs.rs/ym2149-arkos-replayer) |
| [`ym2149-ay-replayer`](crates/ym2149-ay-replayer) | ZXAY/EMUL AY file parser with integrated Z80 replayer | [crates.io/crates/ym2149-ay-replayer](https://crates.io/crates/ym2149-ay-replayer) | [docs.rs/ym2149-ay-replayer](https://docs.rs/ym2149-ay-replayer) |
| [`bevy_ym2149`](crates/bevy_ym2149) | Bevy audio plugin (playback, playlists, diagnostics, audio bridge) | [crates.io/crates/bevy_ym2149](https://crates.io/crates/bevy_ym2149) | [docs.rs/bevy_ym2149](https://docs.rs/bevy_ym2149) |
| [`bevy_ym2149_viz`](crates/bevy_ym2149_viz) | Optional visualization systems & UI builders | [crates.io/crates/bevy_ym2149_viz](https://crates.io/crates/bevy_ym2149_viz) | [docs.rs/bevy_ym2149_viz](https://docs.rs/bevy_ym2149_viz) |
| [`bevy_ym2149_examples`](crates/bevy_ym2149_examples) | Runnable Bevy demos (basic, advanced, crossfade, feature showcase, demoscene, playlist UI) | Workspace-only | [crates/bevy_ym2149_examples/README.md](crates/bevy_ym2149_examples/README.md) |
| [`ym2149-wasm`](crates/ym2149-wasm) | WebAssembly bindings for browser playback ([web demo](https://slippyex.github.io/ym2149-rs/)) | Workspace-only | [crates/ym2149-wasm/README.md](crates/ym2149-wasm/README.md) |
| [`ym2149-bevy`](crates/ym2149-bevy) | Legacy re-export (shim to `bevy_ym2149`) | [crates.io/crates/ym2149-bevy](https://crates.io/crates/ym2149-bevy) | – |

Naming: Bevy-focused crates follow `bevy_ym2149_*`, while core/backends/replayers use the `ym2149-*` prefix.

> **Why Arkos Tracker?**  
> It marries the classic step-sequencer workflow with modern comforts:
> multiple YM2149/AY PSGs per song, visual instrument designers,
> blended software/hardware envelopes, and native export pipelines (like
> this repo). Perfect if you want authentic 8-bit character without
> giving up on flexible tooling.

Arkos playback here is powered by a native Rust replayer—no external
tracker runtime or C++ bindings required.

<img src="docs/screenshots/advanced_example.png" alt="Advanced Bevy example" width="780">

## Highlights

- ✅ **Hardware-faithful**: precise envelope, noise, mixer, SID, Sync Buzzer, digi-drum behaviours
- 📁 **ZXAY/EMUL AY**: bundled replayer with Z80 CPU emulation for the Project AY catalogue
- 🧰 **CLI ready**: stream YM/AKS/AY files in the terminal with real-time visualization
- 🎵 **Native Bevy audio**: seamless integration via `Decodable` trait with pull-based sample generation
- 🛰️ **Configurable Bevy subsystems**: playlists, crossfade decks, music state graphs, channel events, diagnostics, audio bridge
- 🖼️ **Visualization stack**: drop-in oscilloscope, spectrum bars, progress HUD, and demoscene showcase based on the viz crate
- 🧪 **Well-tested**: `cargo test --workspace` (165+ tests) plus example scenes to validate runtime flows
- 🪄 **Gameplay hooks**: Bevy plugin ships marker events, audio-reactive metrics, and PSG one-shot SFX events

## Quick Start

### Use the Core Library

```toml
[dependencies]
# Core emulator only (minimal dependencies)
ym2149 = "0.6.1"

# With streaming audio output
ym2149 = { version = "0.6.1", features = ["streaming"] }

# YM file parsing and playback
ym2149-ym-replayer = "0.6.1"
```

```rust
use ym2149_ym_replayer::{load_song, PlaybackController};

fn main() -> anyhow::Result<()> {
    let data = std::fs::read("song.ym")?;
    let (mut player, summary) = load_song(&data)?;

    player.play()?;
    let samples = player.generate_samples(summary.samples_per_frame as usize);
    println!("{} frames • {} samples", summary.frame_count, samples.len());
    Ok(())
}
```

### Run the CLI Player

```bash
# Real-time playback with scope overlay
cargo run -p ym2149-ym-replayer-cli -- examples/ND-Toxygene.ym

# Interactive chip demo with audio output
cargo run --example chip_demo -p ym2149 --features streaming
```

<img src="docs/screenshots/cli.png" alt="CLI player" width="700">

### Export to Audio Files

```rust
use ym2149_ym_replayer::{load_song, export::export_to_wav_default, export::export_to_mp3_with_config, export::ExportConfig};

fn main() -> anyhow::Result<()> {
    let data = std::fs::read("song.ym")?;
    let (mut player, info) = load_song(&data)?;

    // Export to WAV (feature: export-wav)
    export_to_wav_default(&mut player, info.clone(), "output.wav")?;

    // Export to MP3 with normalization and fade-out (feature: export-mp3)
    let config = ExportConfig::stereo().normalize(true).fade_out(2.0);
    export_to_mp3_with_config(&mut player, "output.mp3", info, 192, config)?;

    Ok(())
}
```

### Add the Bevy Plugin

```rust
use bevy::prelude::*;
use bevy_ym2149::{Ym2149Playback, Ym2149Plugin};
use bevy_ym2149_viz::Ym2149VizPlugin;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, Ym2149Plugin::default(), Ym2149VizPlugin::default()))
        .add_systems(Startup, |mut commands: Commands| {
            commands.spawn(Camera2d);
            commands.spawn(Ym2149Playback::new("assets/music/song.ym")).insert(Name::new("Tracker"));
        })
        .run();
}
```

Need a reference scene? `cargo run --example advanced_example -p bevy_ym2149_examples`.  
Want to try the browser demo? Open https://slippyex.github.io/ym2149-rs/web/simple-player.html (auto-built via GitHub Pages).

## Documentation & Guides

- `crates/ym2149-core/README.md` – emulator architecture, feature flags, CLI/export instructions
- `crates/bevy_ym2149/README.md` – plugin subsystems, playlists, music state graph, audio bridge, diagnostics
- `crates/bevy_ym2149_viz/README.md` – visualization builders and systems
- `crates/bevy_ym2149_examples/README.md` – example matrix + screenshot gallery (incl. playlist crossfade UI)
- [ARCHITECTURE.md](ARCHITECTURE.md) – YM + Arkos playback pipelines and layering details
- [crates/ym2149-core/STREAMING_GUIDE.md](crates/ym2149-core/STREAMING_GUIDE.md) – low-latency streaming details
- `examples/arkos/` – curated Arkos Tracker `.ym/.aks` files for regression tests and the wasm demo

Need to refresh the wasm demo bundle? Run `scripts/build-wasm-examples.sh`
from the repo root to rebuild via `wasm-pack` and copy the output into
`crates/ym2149-wasm/examples/pkg/`.

## Testing

```bash
# Entire workspace
cargo test --workspace

# Focus a crate
cargo test -p ym2149
cargo test -p bevy_ym2149

# Feature-specific tests
cargo test -p ym2149 --features streaming
```

## Development Prerequisites

- Rust 1.83+ (Rust 2024 edition)
- libmp3lame for `export-mp3`
- A working audio backend (cpal/rodio) for streaming playback

## Project Structure

```
ym2149-rs/
├── crates/
│   ├── ym2149-core/          # Core YM2149 chip emulator (published as `ym2149`)
│   ├── ym2149-arkos-replayer/# Arkos Tracker parser + pure-Rust replayer
│   ├── ym2149-ym-replayer/   # YM file parsing and playback
│   ├── ym2149-ym-replayer-cli/# CLI for YM/AKS playback and export
│   ├── ym2149-softsynth/     # Experimental synthesizer backend
│   ├── ym2149-wasm/          # WebAssembly bindings
│   ├── bevy_ym2149/          # Bevy audio plugin
│   ├── bevy_ym2149_viz/      # Visualization helpers
│   ├── bevy_ym2149_examples/ # Runnable Bevy demos
│   └── ym2149-bevy/          # Legacy shim
├── examples/                 # YM sample files
├── docs/                     # Web player (GitHub Pages)
├── Cargo.toml                # Workspace configuration
└── README.md                 # You are here
```

### Deploying the Web Player

The web player is automatically deployed to GitHub Pages via CI/CD:

1. **Enable GitHub Pages** in your repository settings:
   - Go to Settings → Pages
   - Source: "GitHub Actions"

2. **Push to main/master** - the workflow will:
   - Build WASM with `wasm-pack`
   - Copy files to `docs/`
   - Deploy to GitHub Pages

3. **Local testing**:
   ```bash
   cd crates/ym2149-wasm/examples
   ./start-server.sh
   # Open http://localhost:8000/
   ```

## Contributing

Contributions are welcome! Please ensure:
- `cargo fmt` + `cargo clippy`
- `cargo test --workspace`
- Documentation and examples updated for new features

## License

MIT License – see [LICENSE](LICENSE).

## Credits

- **Leonard/Oxygene** – YM format specification & ST-Sound reference material
- **Atari ST + demoscene community** – for the original tunes and docs
- **Rust audio and Bevy ecosystems** – rodio/cpal, Bevy ECS, and community inspiration
