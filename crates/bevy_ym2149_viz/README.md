# bevy_ym2149_viz – YM2149 Visualization Helpers

[![Crates.io](https://img.shields.io/crates/v/bevy_ym2149_viz.svg)](https://crates.io/crates/bevy_ym2149_viz)
[![Docs.rs](https://docs.rs/bevy_ym2149_viz/badge.svg)](https://docs.rs/bevy_ym2149_viz)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](../../LICENSE)

UI companion crate for [`bevy_ym2149`](../bevy_ym2149). It wires oscilloscope traces, spectrum bars, progress HUDs, and playlist indicators into your Bevy scene so you can focus on audio/gameplay logic.

<img src="../../docs/screenshots/advanced_example.png" alt="Visualization layout" width="780">

## Features

- `Ym2149VizPlugin` – registers the systems/resources needed to keep UI widgets in sync with `Ym2149Playback`
- Builder helpers (`create_status_display`, `create_channel_visualization`, `create_detailed_channel_display`, `create_oscilloscope`) for instant layouts
- Component types (`SongInfoDisplay`, `SpectrumBar`, `OscilloscopePoint`, `SongProgressFill`, …) are public so custom UIs can reuse the same systems
- Systems (`update_song_info`, `update_oscilloscope`, `update_song_progress`, etc.) update nodes based on playback + channel snapshots

## Usage

```rust
use bevy::prelude::*;
use bevy_ym2149::{Ym2149Playback, Ym2149Plugin};
use bevy_ym2149_viz::{
    create_status_display, create_channel_visualization, Ym2149VizPlugin,
};

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, Ym2149Plugin::default(), Ym2149VizPlugin::default()))
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.spawn(Ym2149Playback::new("assets/music/ND-Toxygene.ym"));

    create_status_display(&mut commands);
    create_channel_visualization(&mut commands, 3);
}
```

Need more elaborate UI? Inspect the `advanced_example` or `demoscene` demos in `bevy_ym2149_examples`.

## License

MIT – matches the rest of the workspace.
