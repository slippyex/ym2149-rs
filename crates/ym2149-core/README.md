# ym2149 – Cycle-Accurate YM2149 PSG Emulator

[![Crates.io](https://img.shields.io/crates/v/ym2149.svg)](https://crates.io/crates/ym2149)
[![Docs.rs](https://docs.rs/ym2149/badge.svg)](https://docs.rs/ym2149)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](../../LICENSE)

Modern Rust toolkit for Yamaha YM2149 playback: cycle-accurate emulation, full YM format parsing (YM2–YM6 + YMT tracker), real-time streaming, and WAV/MP3 export utilities.

## Feature Highlights

| Area | Details |
|------|---------|
| **Emulation** | Integer-accurate tone/noise/envelope pipeline, SID & Sync Buzzer, digi-drums, DC filter, per-channel mute |
| **Formats** | YM2, YM3, YM3b, YM4, YM5, YM6, YMT1/2 with automatic LHA decompression + metadata extraction |
| **Audio Output** | Real-time streaming via rodio/cpal, buffer rendering, WAV export (hound), MP3 export (libmp3lame), configurable sample rates/stereo gains |
| **Advanced** | Experimental softsynth backend, terminal visualization helpers, effect decoder API, replayer metrics |

## Install

```toml
[dependencies]
ym2149 = { version = "0.6", features = ["emulator", "ym-format", "replayer"] }
```

Enable additional features as needed (see below).

## Quick Start

```rust
use ym2149::replayer::load_song;

fn main() -> anyhow::Result<()> {
    let data = std::fs::read("song.ym")?;
    let (mut player, info) = load_song(&data)?;

    player.play()?;
    let samples = player.generate_samples(44_100);
    println!("{} • {} samples", info.song_name, samples.len());
    Ok(())
}
```

## CLI Player & Export Tool

```bash
# Real-time playback + visualization (requires `streaming`)
cargo run -p ym2149 --features streaming -- examples/ND-Toxygene.ym

# Softsynth experiment
cargo run -p ym2149 --features "streaming softsynth" -- --chip softsynth song.ym

# Export helper (WAV / MP3 / batch)
cargo run -p ym2149 --example export --features export-wav -- song.ym out.wav
cargo run -p ym2149 --example export --features export-mp3 -- song.ym out.mp3 --bitrate 256
cargo run -p ym2149 --example export --features export-wav -- --batch input/ output/
```

<img src="../../docs/screenshots/cli.png" alt="Terminal player" width="720">

## Feature Flags

| Feature | Description |
|---------|-------------|
| `emulator` (default) | Core chip implementation |
| `ym-format` (default) | YM parser/loader |
| `replayer` (default) | Playback engine & effects |
| `streaming` | Real-time audio output (rodio) |
| `visualization` | Terminal UI helpers |
| `softsynth` | Experimental synth backend |
| `export-wav` | WAV rendering (hound) |
| `export-mp3` | MP3 rendering (libmp3lame) |
| `export` | Convenience meta-feature (`export-wav` + `export-mp3`) |

Combine them to keep dependency footprints minimal.

## Format Support

| Format | Support | Notes |
|--------|---------|-------|
| YM2 | ✅ | Mad Max digi-drums, special effects |
| YM3 / YM3b | ✅ | Register dump + loop metadata |
| YM4 | ✅ | Metadata, digi-drums |
| YM5 | ✅ | SID voices, Sync Buzzer, DigiDrums |
| YM6 | ✅ | Extended effects, improved compression |
| YMT1 / YMT2 | ✅ | Tracker formats |

All variants support automatic LHA/LZH decompression, metadata extraction, and loop handling.

## Performance & Metrics

- Sample generation: ~0.5–1 ms per 50 Hz frame (44.1 kHz output)
- Memory: ~1–2 MB per player instance
- Export speed: ~4–5 s for an 8-minute track (WAV/MP3)
- Streaming latency: <5 ms with `StreamConfig::low_latency`

`ym2149::replayer::PlaybackMetrics` exposes frame rate, buffer fill, and effect usage for profiling.

## Examples

The crate ships the `export` example at `crates/ym2149-core/examples/export.rs`, demonstrating WAV/MP3 rendering, fades, normalization, and batch conversion. Run it with `cargo run --example export --features export-wav`.

## Documentation

- API reference: [docs.rs/ym2149](https://docs.rs/ym2149)
- Emulator internals: [`ARCHITECTURE.md`](../../ARCHITECTURE.md)
- Streaming deep dive: [`STREAMING_GUIDE.md`](../../STREAMING_GUIDE.md)

## Contributing

Please run `cargo fmt`, `cargo clippy`, and `cargo test -p ym2149 --all-features` before submitting changes. See the workspace root README for global contribution guidelines.

## License

MIT License – see [LICENSE](../../LICENSE).
