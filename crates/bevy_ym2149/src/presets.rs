//! Convenience helpers to wire common YM2149 plugin stacks.
use bevy::prelude::*;

use crate::plugin::Ym2149Plugin;

/// Adds the core YM2149 audio plugin with all subsystems enabled.
pub fn add_audio_stack(app: &mut App) {
    app.add_plugins(Ym2149Plugin::default());
}

/// Adds YM2149 audio. For audio + viz, use `bevy_ym2149_viz::add_full_stack`.
pub fn add_full_stack(app: &mut App) {
    add_audio_stack(app);
}
