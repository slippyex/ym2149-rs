# Changelog

All notable changes to the ym2149-rs project.

## [v0.6.0] - Current Release

### Added
- Arkos Tracker support end-to-end: `ym2149-arkos-replayer` crate, Bevy/wrapper integration, curated fixtures in `examples/arkos`, wasm auto-detection for `.aks`
- New `scripts/build-wasm-examples.sh` helper to rebuild/copy the wasm bundle for demos & releases
- Message-based Bevy runtime: `FrameAudioData`, split `initialize_playback` / `drive_playback_state` / `process_playback_frames` + diagnostics and bridge consumers

### Changed
- `Ym2149Plugin` now accepts `.aks` sources transparently; docs/README updated accordingly
- wasm `Ym2149Player` automatically falls back to Arkos player when YM parsing fails, exposing the same JavaScript API
- Documentation refresh (`README.md`, `ARCHITECTURE.md`, crate READMEs) to reflect Arkos fixtures, new Bevy systems, wasm workflow

### Fixed
- Removed dependency on the upstream `arkostracker3` repo by copying reference `.aks/.ym` fixtures into `examples/arkos`
- Optional `extended-tests` now run against local fixtures; parser tests no longer panic when the Arkos repo is absent


### New Crates

- **bevy_ym2149_examples** - Comprehensive example suite (basic, advanced, feature_showcase, crossfade, demoscene)
- **bevy_ym2149_viz** - Optional visualization companion crate (extracted from core plugin)

### New Features

- **Playlist System** - `.ymplaylist` assets with automatic track progression and multiple playback modes
- **Playlist Crossfading** - Dual-deck blending that starts at 90% by default, supports fixed-time triggers, and now accepts explicit overlap window durations
- **bevy_ym2149_viz** - Visualization helpers extracted into a dedicated crate with `Ym2149VizPlugin`
- **Music State Graph** - Named state machine for dynamic soundtrack transitions
- **Audio Bridge** - Mirror generated samples into Bevy's audio graph with per-entity gain/pan control
- **Channel Events** - Per-frame channel snapshots and lifecycle events (TrackStarted/TrackFinished)
- **Diagnostics Integration** - Buffer fill and frame position metrics via Bevy's diagnostics system
- **Plugin Configuration** - `Ym2149PluginConfig` for toggling subsystems (playlists, events, diagnostics, etc.)
- **Enhanced Playback** - Added `from_asset()` and `from_bytes()` constructors for flexible loading

### Architecture Changes

- Split monolithic plugin into modular structure (plugin/mod.rs, plugin/config.rs, plugin/systems.rs)
- Extracted visualization to separate `bevy_ym2149_viz` crate for optional UI dependencies
- Added comprehensive error handling with `Ym2149Error` enum
- New modules: audio_bridge, diagnostics, error, events, music_state, playlist

### Documentation

- Expanded README from ~100 to 623 lines with comprehensive API guide
- Added example crate README with asset configuration details
- Added visualization crate README
- Moved core architecture docs to `ym2149-core/ARCHITECTURE.md`

### Examples & Assets

- 5 runnable examples demonstrating all plugin features
- 6 YM music files and custom demoscene bitmap font
- WGSL shaders for raymarched scenes and CRT post-processing

### Testing

- Added integration test suite (550+ lines covering playback and plugin lifecycle)
- Added playlist crossfade scheduler tests (triggering, completion, and TrackFinished suppression)
- Added `bevy_ym2149_viz` crate-level coverage via the advanced example

### Breaking Changes

- Visualization moved to `bevy_ym2149_viz` crate - requires separate dependency and import
- `Ym2149PluginConfig::visualization` removed; add `Ym2149VizPlugin` explicitly when UI is desired
- Advanced features require explicit opt-in via `Ym2149PluginConfig`


---

## [0.5.1] - Previous Release

Initial stable release of bevy_ym2149 plugin.
