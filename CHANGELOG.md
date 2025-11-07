# Changelog

All notable changes to the ym2149-rs project.

## [Unreleased] - v0.next

### New Crates

- **bevy_ym2149_examples** - Comprehensive example suite (basic, advanced, feature_showcase, crossfade, demoscene)
- **bevy_ym2149_viz** - Optional visualization companion crate (extracted from core plugin)

### New Features

- **Playlist System** - `.ymplaylist` assets with automatic track progression and multiple playback modes
- **Playlist Crossfading** - Dual-deck blending that starts at 90% by default, supports fixed-time triggers, and now accepts explicit overlap window durations
- **Music State Graph** - Named state machine for dynamic soundtrack transitions
- **Audio Bridge** - Mirror generated samples into Bevy's audio graph with per-entity gain/pan control
- **Channel Events** - Per-frame channel snapshots and lifecycle events (TrackStarted/TrackFinished)
- **Diagnostics Integration** - Buffer fill and frame position metrics via Bevy's diagnostics system
- **Spatial Audio** - Experimental stereo panning based on Bevy transforms (opt-in)
- **Plugin Configuration** - `Ym2149PluginConfig` for toggling subsystems (playlists, events, diagnostics, etc.)
- **Enhanced Playback** - Added `from_asset()` and `from_bytes()` constructors for flexible loading

### Architecture Changes

- Split monolithic plugin into modular structure (plugin/mod.rs, plugin/config.rs, plugin/systems.rs)
- Extracted visualization to separate `bevy_ym2149_viz` crate for optional UI dependencies
- Added comprehensive error handling with `Ym2149Error` enum
- New modules: audio_bridge, diagnostics, error, events, music_state, playlist, spatial

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

### Breaking Changes

- Visualization moved to `bevy_ym2149_viz` crate - requires separate dependency and import
- Advanced features require explicit opt-in via `Ym2149PluginConfig`

### Statistics

- 52 files changed: +8,245 / -1,018 lines
- 11 commits

---

## [0.5.1] - Current Release

Initial stable release of bevy_ym2149 plugin.
