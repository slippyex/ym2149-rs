# ym2149-ym-replayer

YM file format parser and music replayer for YM2149 PSG chips.

This crate provides comprehensive support for parsing and playing back YM chiptune files, including YM2/3/5/6 formats, tracker modes, and various hardware effects.

## Features

- **YM Format Support**: YM2, YM3, YM5, YM6 file formats with LHA decompression
- **Tracker Modes**: YMT1 and YMT2 tracker format support
- **Format Profiles**: `FormatProfile` trait encapsulates format quirks (YM2 drum mixing, YM5 effect encoding, YM6 sentinel handling) so new formats plug in without bloating `Ym6PlayerGeneric`
- **Frame Sequencer**: Dedicated `FrameSequencer` stores frames + timing and exposes seek/loop APIs
- **Effects Pipeline**: `EffectsPipeline` wraps the low-level `EffectsManager`, tracking SID/digidrum state for visualization/metadata
- **Hardware Effects**:
  - Mad Max digi-drums
  - YM6 SID voice effects
  - Sync buzzer effects
- **Backend Agnostic**: Works with any `Ym2149Backend` implementation
- **Optional Features**: Streaming audio output

## Install

Add the crate to your `Cargo.toml`:

```toml
[dependencies]
ym2149-ym-replayer = "0.7"
```

## Usage

### Basic Playback

```rust
use ym2149_ym_replayer::{load_song, ChiptunePlayer, PlaybackMetadata};

let data = std::fs::read("song.ym")?;
let (mut player, summary) = load_song(&data)?;

// Use the unified ChiptunePlayer interface
player.play();
let samples = player.generate_samples(summary.samples_per_frame as usize);

// Access metadata
println!("{} by {}", player.metadata().title(), player.metadata().author());
```

### Loading from Files

```rust
use ym2149_ym_replayer::loader;

// From file path
let frames = loader::load_file("song.ym")?;

// From bytes
let data = std::fs::read("song.ym")?;
let frames = loader::load_bytes(&data)?;
```

### Format Profiles & Effects Pipeline

Internally the player is split into three layers:

1. **FrameSequencer** – Owns the register frames, loop point, and PAL/NTSC timing and offers seek/loop APIs.
2. **FormatProfile** – Trait implemented per format (YM2/YM5/YM6/basic) so register preprocessing and effect decoding live behind `FormatMode` strategies instead of `is_ym*_mode` flags.
3. **EffectsPipeline** – Wraps the low-level `EffectsManager`, tracks which SID/digidrum voices are active, and feeds the backend every sample.

`load_song` automatically selects the right profile, and custom loaders can create a profile via `ym2149_ym_replayer::player::create_profile`. Metadata and Bevy visualizers now query effect state through the pipeline.

## Architecture

This crate was extracted from `ym2149-core` to provide better separation of concerns:

- **ym2149-core**: Pure YM2149 chip emulation
- **ym2149-ym-replayer**: YM file parsing and playback (this crate)
- **ym2149-softsynth**: Experimental synthesizer backend (workspace helper; not published)

## Migration from ym2149-core < 0.6

If you were using the deprecated modules from `ym2149-core`:

```rust
// Old (deprecated)
use ym2149::replayer::Ym6Player;
use ym2149::ym_loader;

// New
use ym2149_ym_replayer::Ym6Player;
use ym2149_ym_replayer::loader;
```

## Feature Flags

- `default`: Includes effects, tracker, and digi-drums support
- `effects`: Enable YM6 effect processing
- `tracker`: Enable tracker mode support
- `digidrums`: Enable Mad Max digi-drums
- `streaming`: Enable real-time audio output (requires `rodio`) - for CLI/standalone use; Bevy integration uses native audio
- `export-wav`: Enable WAV file export (requires `hound`)
- `softsynth`: Workspace-only hook for experimental software synthesizer backends (requires providing your own `ym2149-softsynth` via `[patch]`)

> MP3 export was removed because the LAME/Autotools toolchain is fragile across environments. Export WAV and transcode externally (e.g., `ffmpeg`).

## License

See the main ym2149-rs repository for license information.
