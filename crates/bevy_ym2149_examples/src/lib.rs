//! Shared utilities for bevy_ym2149 examples
//!
//! This module provides common configuration and helper functions used across
//! all example applications to reduce duplication and ensure consistency.

use bevy::app::PluginGroup;
use bevy::asset::AssetPlugin;
use bevy::prelude::DefaultPlugins;
use bevy_embedded_assets::{EmbeddedAssetPlugin, PluginMode};

/// Asset base path resolved at compile time to ensure examples work from any directory.
///
/// This path points to the examples crate's asset directory, allowing examples
/// to be run from the workspace root, crate directory, or as standalone binaries.
///
/// # Design Rationale
/// - `CARGO_MANIFEST_DIR` is resolved at compile time to the crate root
/// - Works regardless of current working directory at runtime
/// - Consistent with Bevy's expectation of an "assets" directory
/// - Enables drag-and-drop file loading alongside bundled assets
///
/// # Example
/// ```
/// const ASSET_BASE: &str = bevy_ym2149_examples::ASSET_BASE;
/// // Use in asset paths: "music/ND-Toxygene.ym"
/// ```
pub const ASSET_BASE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets");

/// Returns a configured [`EmbeddedAssetPlugin`] that bundles all example assets into the binary.
///
/// The plugin replaces the default Bevy asset source and falls back to the on-disk `ASSET_BASE`
/// when assets are missing (useful during development). Add this plugin *before* the default
/// Bevy plugins.
pub fn embedded_asset_plugin() -> EmbeddedAssetPlugin {
    EmbeddedAssetPlugin {
        mode: PluginMode::ReplaceAndFallback {
            path: ASSET_BASE.into(),
        },
    }
}

/// Configure DefaultPlugins with the correct asset path for examples.
///
/// This helper ensures all examples use consistent asset loading configuration,
/// reducing boilerplate and preventing configuration drift.
///
/// # Example
///
/// ```no_run
/// use bevy::prelude::*;
/// use bevy_ym2149_examples::example_plugins;
///
/// App::new()
///     .add_plugins(example_plugins())
///     .run();
/// ```
pub fn example_plugins() -> impl PluginGroup {
    DefaultPlugins.set(AssetPlugin {
        file_path: ASSET_BASE.into(),
        ..Default::default()
    })
}
