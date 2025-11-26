# ym2149-common

Shared traits and types for YM2149 chiptune replayers.

## Overview

This crate provides the unified interface that all replayer crates (`ym2149-ym-replayer`, `ym2149-arkos-replayer`, `ym2149-ay-replayer`) implement. It enables writing format-agnostic code that works with any chiptune player.

## Key Types

### `ChiptunePlayer` trait

Common playback interface for all player types:

```rust
use ym2149_common::{ChiptunePlayer, PlaybackState};

fn play_any<P: ChiptunePlayer>(player: &mut P) {
    player.play();

    // Generate audio samples
    let mut buffer = vec![0.0f32; 882]; // ~20ms at 44.1kHz
    player.generate_samples_into(&mut buffer);

    // Check playback state
    if player.state() == PlaybackState::Playing {
        println!("Currently playing: {}", player.metadata().title());
    }
}
```

### `PlaybackMetadata` trait

Unified metadata access across all formats:

```rust
use ym2149_common::PlaybackMetadata;

fn show_info<M: PlaybackMetadata>(meta: &M) {
    println!("Title:  {}", meta.title());
    println!("Author: {}", meta.author());
    println!("Format: {}", meta.format());

    if let Some(frames) = meta.frame_count() {
        let duration = frames as f32 / meta.frame_rate() as f32;
        println!("Duration: {:.1}s", duration);
    }
}
```

### `PlaybackState` enum

Standard playback states used by all players:

```rust
use ym2149_common::PlaybackState;

match player.state() {
    PlaybackState::Stopped => println!("Stopped"),
    PlaybackState::Playing => println!("Playing"),
    PlaybackState::Paused  => println!("Paused"),
}
```

### `BasicMetadata` struct

A concrete metadata type for simple use cases:

```rust
use ym2149_common::BasicMetadata;

let meta = BasicMetadata {
    title: "My Song".into(),
    author: "Composer".into(),
    format: "YM".into(),
    frame_count: Some(5000),
    frame_rate: 50,
    ..Default::default()
};
```

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
ym2149-common = "0.7"
```

All replayer crates re-export these types, so you typically don't need to depend on `ym2149-common` directly:

```rust
// These are equivalent:
use ym2149_common::ChiptunePlayer;
use ym2149_ym_replayer::ChiptunePlayer;
use ym2149_arkos_replayer::ChiptunePlayer;
use ym2149_ay_replayer::ChiptunePlayer;
```

## License

MIT
