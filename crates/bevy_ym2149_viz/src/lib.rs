#![warn(missing_docs)]

//! Visualization utilities for `bevy_ym2149`.
//!
//! This crate hosts all UI components, helpers, and Bevy systems that render
//! oscilloscope, spectrum, and song-status information based on the data exposed
//! by the core audio plugin. Applications should add `Ym2149VizPlugin` alongside
//! `Ym2149Plugin` to enable the widgets and use the builder helpers to spawn
//! their preferred UI layout.

mod builders;
mod components;
mod helpers;
mod stack;
mod systems;
mod uniforms;

use bevy::prelude::*;

pub use builders::{
    create_channel_visualization, create_detailed_channel_display, create_oscilloscope,
    create_song_info_display, create_status_display,
};
pub use components::*;
pub use stack::add_full_stack;
pub use systems::{
    update_detailed_channel_display, update_oscilloscope, update_song_info, update_song_progress,
    update_status_display,
};
pub use uniforms::{OscilloscopeUniform, SpectrumUniform};

/// Plugin that wires the visualization resources and systems into a Bevy app.
#[derive(Default)]
pub struct Ym2149VizPlugin;

impl Plugin for Ym2149VizPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<bevy_ym2149::OscilloscopeBuffer>();
        app.init_resource::<OscilloscopeUniform>();
        app.init_resource::<SpectrumUniform>();

        app.add_systems(
            Update,
            (
                systems::update_song_info,
                systems::update_status_display,
                systems::update_detailed_channel_display,
                systems::update_song_progress,
                systems::update_oscilloscope,
            ),
        );
    }
}
