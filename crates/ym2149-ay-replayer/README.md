# ym2149-ay-replayer – ZXAY/EMUL player with Z80 emulation

Cycle-accurate Project AY (`.ay`) playback for Rust. This crate parses
the original **ZXAY/EMUL** container, reconstructs the signed
pointer-based structures (header, subsongs, points, block tables) and
runs the embedded Z80 player inside a pure Rust environment powered by
[`iz80`](https://crates.io/crates/iz80) and the workspace’s
[`ym2149` chip](../ym2149-core).

> Project AY delivered thousands of Spectrum and CPC rips that bundle a
> tiny Z80 replay as part of the file. Instead of shipping bespoke
> shims, this crate emulates that player verbatim – including the
> Motorola-endian headers, signed block offsets, and port wiring for
> ZX/CPC machines.

## Highlights

- 🧾 **ZXAY parser** – validates header signatures, extracts metadata,
  subsongs, NT strings, and memory block layouts.
- 🧠 **Z80 execution** – runs the bundled player using the iz80 core,
  including INIT/INTERRUPT entries, stack setup, register presets, and
  per-frame interrupts.
- 🎹 **Real PSG bridge** – wired to the shared `ym2149` backend so the
  CLI, Bevy plugin, exporter, and wasm builds all hear the same output.
- 🕹 **CPC + Spectrum** – detects PPI-style port access (`#F4xx/#F6xx`)
  and re-tunes the PSG clock for 1 MHz CPC rips while keeping 2 MHz for
  ZX files.
- 📦 **ProjectAY fixtures** – unit tests load real songs
  (`SpaceMadness.AY`, `impact demo 3_2.ay`) to guard against parser or
  emulator regressions.
- 🔌 **Reusable player** – `AyPlayer` implements the same
  `RealtimeChip`/`PlaybackController` traits as the YM and Arkos
  players, so downstream code just boxes one more variant.

## Quick Start

```toml
[dependencies]
ym2149-ay-replayer = "0.9"
# Optional but common when you plan to pipe the samples into the rest of the stack
ym2149 = "0.9"
```

```rust
use ym2149_ay_replayer::{load_ay, AyPlayer};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = std::fs::read("ProjectAY/Spectrum/Demos/SpaceMadness.AY")?;
    let ay = load_ay(&data)?;
    println!("File version: {}", ay.header.file_version);
    println!("Songs: {}", ay.songs.len());

    let (mut player, meta) = AyPlayer::load_from_bytes(&data, ay.header.first_song_index as usize)?;
    println!("Playing: {}", meta.song_name);

    player.play()?;
    let samples = player.generate_samples(882); // ≈ one 50 Hz frame @ 44.1 kHz
    println!("Rendered {} samples", samples.len());
    Ok(())
}
```

### Firmware Limitations

The player only handles AY drivers that run entirely out of their own
code/data; once a track calls into the Spectrum or CPC ROM jump table we
stop playback and report that firmware emulation is unsupported. ZX
files that stay self-contained work fine; CPC AY rips and ROM-heavy ZX
rips should be played in a full emulator instead.

### When to use it

- Integrate `.ay` playback into tooling alongside `.ym` and `.aks` files
- Feed AY songs through the CLI (`ym2149-replayer-cli`), Bevy plugin,
  or wasm demo without branching per format
- Inspect AY headers / block layouts when writing conversion pipelines

## API Surface

- [`load_ay`](src/parser.rs) → low-level parser returning `AyFile`
  (header metadata, song list, block descriptors)
- [`AyPlayer`](src/player.rs) → Z80 + PSG player with familiar
  `play/pause/stop/generate_samples` methods
- [`AyMetadata`](src/player.rs) → descriptive info for UIs/inspectors
- [`AyMachine`](src/machine.rs) → host implementation of the AY memory
  map + PSG port bridging

The player mirrors the workspace conventions: it is `Send`, implements
`PlaybackController`, exposes mute toggles, register snapshots, and
playback position helpers.

## Relationship to the Workspace

- **CLI (`ym2149-replayer-cli`)**: file detection now routes `.ym`,
  `.aks`, and `.ay` to the correct player, reusing the same streaming
  + visualization stack.
- **Bevy (`bevy_ym2149`)**: `YmSongPlayer` gained an `Ay` variant so
  assets dropped into Bevy can be YM/AKS/AY without code changes.
- **WASM (`ym2149-wasm`)**: browser player auto-detects AY files and
  uses the same `AyPlayer` under the hood.
- **Docs/Examples**: Project AY fixtures live in `/ProjectAY/` and are
  used by both tests and the stats helper in `examples/stats.rs`.

## Testing

```bash
# Parser + player tests (uses bundled AY fixtures)
cargo test -p ym2149-ay-replayer

# Optional helper to scan fixture stats (interrupt usage, etc.)
cargo run -p ym2149-ay-replayer --example stats -- ProjectAY
```

The fixtures cover both Spectrum and CPC titles so we don’t regress on
port wiring, pointer arithmetic, or INIT/INTERRUPT fallbacks.

Have more AY test material? Drop it under `ProjectAY/` and extend the
unit tests – the more demoscene rips we cover, the safer future
refactors become. Questions/bugs? Open an issue on the main repo. Happy
tuning!

## CPC Firmware Notes

Some `.ay` files target the Amstrad CPC firmware: they talk to the PSG
via the PPI (`#F4xx/#F6xx`) and rely on jump tables, AMSDOS buffers, and
keyboard handlers that live in the CPC ROM/RAM workspace. Emulating that
environment correctly requires bundling the original ROMs and a sizable
portion of the CPC OS, which is well beyond the scope of this crate (and
comes with legal/licencing uncertainties). As a result **CPC-based AY
files are currently not supported**: the player will detect the CPC port
traffic, stop playback, and report that firmware emulation is missing.

ZX Spectrum AY rips continue to work normally; if you need CPC playback,
use a full CPC emulator or a specialised player that ships the firmware.
