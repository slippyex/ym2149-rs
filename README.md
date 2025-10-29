# YM2149‑RS

Cycle‑accurate Yamaha YM2149 PSG (Atari ST) emulator in Rust, with optional YM file replay, real‑time streaming, and an experimental softsynth.

## Highlights

- Integer‑accurate YM2149 core (envelope, LFSR noise, mixer, color filter)
- YM file replay: YM2 (Mad Max), YM3/YM3b, YM4, YM5, YM6
- VBL‑synced replayer with YM5/YM6 effects (SID, Sync Buzzer)
- Optional real‑time streaming (rodio) and experimental softsynth
- Modular features: enable only what you need

## YM File Replay

This crate can replay YM files (YM2–YM6) through a simple frame‑based replayer.
You can render to a buffer or stream in real‑time. Effects encoded in YM5/YM6 are supported; YM2 Mad Max digi‑drums are handled as well.

Minimal example (buffer-based):

```rust
use ym2149::{replayer::PlaybackController, replayer::load_song};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Buffer-based API: pass bytes directly (auto-detects + decompresses)
    let data = std::fs::read("examples/Scaven6.ym")?; // LHA-compressed OK
    let (mut player, _summary) = load_song(&data)?;    // YM2–YM6 supported
    player.play()?;
    let samples = player.generate_samples(44_100);  // render ~1s at 44.1kHz
    println!("{} samples", samples.len());
    Ok(())
}
```

### Loading YM Data (files or buffers)

If you want parsed register frames instead of a player, use the file loader:

```rust
// From a file path (auto-detects format, transparently decompresses LHA)
use ym2149::ym_loader::load_file;
let frames = load_file("examples/Scaven6.ym")?; // Vec<[u8; 16]>
println!("{} frames", frames.len());

// From an in-memory buffer
use ym2149::ym_loader::load_bytes;
let data = std::fs::read("examples/Scaven6.ym")?;
let frames = load_bytes(&data)?;
```

### Real‑Time Streaming (replayer + streaming)

```rust
use ym2149::{replayer::PlaybackController, replayer::load_song};
use ym2149::streaming::{StreamConfig, RealtimePlayer, AudioDevice};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load YM data
    let data = std::fs::read("examples/Scaven6.ym")?;
    let (mut song, summary) = load_song(&data)?; // auto-detect YM2–YM6

    // Start song playback
    song.play()?;

    // Set up streaming
    let cfg = StreamConfig::low_latency(44_100);
    let stream = RealtimePlayer::new(cfg)?;
    let _dev = AudioDevice::new(cfg.sample_rate, cfg.channels, stream.get_buffer())?;

    // Push audio in small batches
    let mut batch = vec![0.0f32; 1024];
    let total = summary.samples_per_frame as usize * summary.frame_count as usize;
    let mut generated = 0usize;
    while generated < total {
        let to_gen = (total - generated).min(batch.len());
        let samples = song.generate_samples(to_gen);
        batch[..to_gen].copy_from_slice(&samples);
        stream.write_blocking(&batch[..to_gen]);
        generated += to_gen;
    }

    Ok(())
}
```

## Quick Start

Add to your `Cargo.toml` (enable only what you need):

```toml
[dependencies]
ym2149 = { version = "0.1", features = ["emulator", "ym-format", "replayer", "streaming"] }
# add `"softsynth"` if you want the experimental synth engine
```

### Core chip (generate samples)

```rust
use ym2149::ym2149::Ym2149;

let mut chip = Ym2149::new();
chip.write_register(0, 0x50); // A freq lo
chip.write_register(1, 0x00); // A freq hi
chip.write_register(8, 0x0F); // A amp
chip.clock();
let sample = chip.get_sample();
```

### Streaming (feature: `streaming`)

```rust
use ym2149::streaming::{StreamConfig, RealtimePlayer, AudioDevice};

let cfg = StreamConfig::low_latency(44_100);
let player = RealtimePlayer::new(cfg)?;
let _dev = AudioDevice::new(cfg.sample_rate, cfg.channels, player.get_buffer())?;
// write samples into player.write_blocking(&samples)
```

## CLI Player

The repository ships a `ym2149` CLI that performs real-time playback with terminal visualization. The binary is only built when the `streaming` feature is enabled:

```bash
cargo run --features streaming -- examples/Scaven6.ym
```

To experiment with the experimental softsynth backend, add the `softsynth` feature and select it via the `--chip` flag:

```bash
cargo run --features "streaming softsynth" -- --chip softsynth examples/Ashtray.ym
```

## Features

- `emulator` (default): core YM2149 chip
- `ym-format` (default): YM file parsing/loader
- `replayer` (default): frame player + effects
- `streaming`: rodio-powered audio output (opt-in)
- `softsynth`: experimental synth engine (opt-in)
- `visualization` (default): terminal visualization helpers

## License

MIT — see `LICENSE`.
