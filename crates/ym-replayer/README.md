# ym-replayer

YM file format parser and music replayer for YM2149 PSG chips.

This crate provides comprehensive support for parsing and playing back YM chiptune files, including YM2/3/5/6 formats, tracker modes, and various hardware effects.

## Features

- **YM Format Support**: YM2, YM3, YM5, YM6 file formats with LHA decompression
- **Tracker Modes**: YMT1 and YMT2 tracker format support
- **Hardware Effects**:
  - Mad Max digi-drums
  - YM6 SID voice effects
  - Sync buzzer effects
- **Backend Agnostic**: Works with any `Ym2149Backend` implementation
- **Optional Features**: Streaming audio output

## Usage

### Basic Playback

```rust
use ym_replayer::{load_song, PlaybackController};

let data = std::fs::read("song.ym")?;
let (mut player, summary) = load_song(&data)?;
player.play()?;

// Generate audio samples
let samples = player.generate_samples(summary.samples_per_frame as usize);
```

### Loading from Files

```rust
use ym_replayer::loader;

// From file path
let frames = loader::load_file("song.ym")?;

// From bytes
let data = std::fs::read("song.ym")?;
let frames = loader::load_bytes(&data)?;
```

## Architecture

This crate was extracted from `ym2149-core` v0.6.0 to provide better separation of concerns:

- **ym2149-core**: Pure YM2149 chip emulation
- **ym-replayer**: YM file parsing and playback (this crate)
- **ym-softsynth**: Experimental synthesizer backend

## Migration from ym2149-core < 0.6

If you were using the deprecated modules from `ym2149-core`:

```rust
// Old (deprecated)
use ym2149::replayer::Ym6Player;
use ym2149::ym_loader;

// New
use ym_replayer::Ym6Player;
use ym_replayer::loader;
```

## Feature Flags

- `default`: Includes effects, tracker, and digi-drums support
- `effects`: Enable YM6 effect processing
- `tracker`: Enable tracker mode support
- `digidrums`: Enable Mad Max digi-drums
- `streaming`: Enable real-time audio output (requires `rodio`) - for CLI/standalone use; Bevy integration uses native audio
- `export-wav`: Enable WAV file export (requires `hound`)
- `export-mp3`: Enable MP3 file export (requires `mp3lame-encoder`)
- `softsynth`: Enable experimental software synthesizer backend

## License

See the main ym2149-rs repository for license information.
