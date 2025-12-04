//! Bevy diagnostics integration for YM2149 playback monitoring.
//!
//! This module provides diagnostic paths for tracking playback metrics
//! through Bevy's built-in diagnostics system.

use crate::playback::Ym2149Playback;
use crate::plugin::Ym2149PluginConfig;
use bevy::diagnostic::{Diagnostic, DiagnosticPath, Diagnostics, RegisterDiagnostic};
use bevy::prelude::*;

/// Diagnostic path for audio buffer fill level (reserved for future use).
pub const BUFFER_FILL_PATH: DiagnosticPath = DiagnosticPath::const_new("ym2149/buffer_fill");

/// Diagnostic path for current frame position across all playbacks.
pub const FRAME_POSITION_PATH: DiagnosticPath = DiagnosticPath::const_new("ym2149/frame_position");

/// Register YM2149 diagnostics with the Bevy app.
pub fn register(app: &mut App) {
    app.register_diagnostic(Diagnostic::new(BUFFER_FILL_PATH));
    app.register_diagnostic(Diagnostic::new(FRAME_POSITION_PATH));
}

/// System that updates diagnostic measurements each frame.
pub fn update_diagnostics(
    config: Res<Ym2149PluginConfig>,
    mut diagnostics: Diagnostics,
    playbacks: Query<&Ym2149Playback>,
) {
    if !config.diagnostics {
        return;
    }

    let mut max_frame = 0.0f64;

    for playback in playbacks.iter() {
        max_frame = max_frame.max(playback.frame_position() as f64);
    }

    // TODO: Buffer fill diagnostic not available with Bevy audio
    // Could potentially track via AudioPlayer stats if available

    diagnostics.add_measurement(&FRAME_POSITION_PATH, || max_frame);
}
