use crate::playback::Ym2149Playback;
use crate::plugin::Ym2149PluginConfig;
use bevy::diagnostic::{Diagnostic, DiagnosticPath, Diagnostics, RegisterDiagnostic};
use bevy::prelude::*;

pub const BUFFER_FILL_PATH: DiagnosticPath = DiagnosticPath::const_new("ym2149/buffer_fill");
pub const FRAME_POSITION_PATH: DiagnosticPath = DiagnosticPath::const_new("ym2149/frame_position");

pub fn register(app: &mut App) {
    app.register_diagnostic(Diagnostic::new(BUFFER_FILL_PATH));
    app.register_diagnostic(Diagnostic::new(FRAME_POSITION_PATH));
}

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
