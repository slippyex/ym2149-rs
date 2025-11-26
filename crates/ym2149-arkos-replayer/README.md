# ym2149-arkos-replayer

Native Rust implementation of **Arkos Tracker 2 & 3** playback (no C++
bindings). This crate parses `.aks` tracker projects, builds a
multi-PSG timeline, and exposes a high-level
[`ArkosPlayer`](src/player.rs) for desktop tools, Bevy apps, audio
exports, and the wasm/CLI stacks.

> **What is Arkos Tracker?**  
> A modern, cross-platform tracker for YM2149/AY-3-8910 chips with all
> the niceties we wish we had on Atari ST/CPC: multiple PSG banks per
> song, graphical instrument editors, combined software/hardware
> envelopes, and one-click export to native players. It’s a favourite in
> the CPC/Atari community because it keeps the authentic chip sound while
> still being flexible for new productions.

## Highlights

- 🦀 **Pure Rust replayer** – plays Arkos Tracker projects directly (no
  external tracker binaries or FFI bindings)
- ✅ **Full AKS parser** – metadata, instruments, arpeggios, patterns,
  speed tracks, subsongs, and Digi-Drums
- 🎛 **Multi-PSG playback** – arbitrary chip counts with independent
  master clocks (CPC, Atari ST, PlayCity…)
- 🧠 **Accurate effects** – software envelopes, pitch tables, retrigger,
  hardware envelope macros, and per-voice sample players
- 🔌 **Flexible integration** – drive it manually, embed in Bevy
  (`YmSongPlayer::Arkos`), or export via the CLI/wasm stacks
- 🧪 **Parity-tested** – optional `extended-tests` feature runs reference
  comparisons against bundled tracker songs

## Quick Start

```toml
[dependencies]
ym2149-arkos-replayer = "0.7"
ym2149 = "0.7"              # required by ArkosPlayer for PSG backends
```

```rust
use ym2149_arkos_replayer::{load_aks, ArkosPlayer};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = std::fs::read("music/Perseverance.aks")?;
    let song = load_aks(&data)?;

    println!("Title  : {}", song.metadata.title);
    println!("Subsongs: {}", song.subsongs.len());

    // Play subsong 0 with the default PSG setup
    let mut player = ArkosPlayer::new(song, 0)?;
    player.play()?;

    // Pull samples (882 samples ≈ one 50 Hz frame)
    let left = player.generate_samples(882);
    println!("Rendered {} samples", left.len());
    Ok(())
}
```

### When to use it

- Load `.aks` tracker projects directly in tooling (visualizers,
  conversion pipelines, custom DAWs)
- Mix Arkos tracks alongside YM songs (see `bevy_ym2149::YmSongPlayer`)
- Validate imported instruments/effects against the tracker itself

## Sample Songs

Need real-world material? The workspace ships a few curated Arkos
Tracker exports under [`examples/arkos/`](../../examples/arkos):

- `Doclands - Pong Cracktro (YM).{aks,ym}`
- `Excellence in Art 2018 - Just add cream.{aks,ym}`
- `Andy Severn - Lop Ears.{aks,ym}`

They power the `extended-tests` parity suite and double as drop-in demos
for the wasm player. Feel free to add more as needed.

## Feature Flags

| Feature          | Default | Description |
|------------------|---------|-------------|
| `effects`        | ❌      | Enables SID / software envelopes / pitch LFO helpers |
| `digidrums`      | ❌      | Includes Digi-Drum sample players |
| `full`           | ❌      | Convenience flag for `["effects", "digidrums"]` |
| `extended-tests` | ❌      | Runs parity tests that require external Arkos fixtures |

Most downstream users only enable the features they need to minimize
compile times. The `extended-tests` flag is intended for CI/regression
runs: it downloads/reads test assets and therefore stays opt-in.

## Testing

```bash
# Fast unit tests
cargo test -p ym2149-arkos-replayer

# Include parity tests that rely on tracker assets
cargo test -p ym2149-arkos-replayer --features extended-tests
```

The parity suite mirrors Arkos Tracker’s own replayer output so we can
detect regressions in instrument macros, channel routing, and PSG
converters.

## Relationship to the rest of the workspace

- `bevy_ym2149` automatically consumes Arkos songs via
  `YmSongPlayer::Arkos`, so Bevy apps can spawn `.aks` files alongside
  YM playbacks.
- `ym2149-replayer-cli` ships an Arkos demo mode where the CLI exposes both
  YM and AKS playback through the same visualization stack.
- All PSG backends in the workspace implement `Ym2149Backend`, so the
  Arkos player can share the execution path with YM files, exports, and
  wasm builds.

Questions or ideas? Open an issue on the main repo or ping us on the
Atari ST / YM2149 Discord channels. Happy tracking!
