use bevy::prelude::*;

use crate::Ym2149VizPlugin;

/// Convenience helper: adds core YM2149 audio plugin and the viz plugin.
pub fn add_full_stack(app: &mut App) {
    app.add_plugins((bevy_ym2149::Ym2149Plugin::default(), Ym2149VizPlugin));
}
