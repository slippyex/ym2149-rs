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

    let mut fill_sum = 0.0f64;
    let mut device_count = 0.0f64;
    let mut max_frame = 0.0f64;

    for playback in playbacks.iter() {
        if let Some(device) = &playback.audio_device {
            fill_sum += device.buffer_fill_level() as f64;
            device_count += 1.0;
        }
        max_frame = max_frame.max(playback.frame_position() as f64);
    }

    if device_count > 0.0 {
        let average_fill = fill_sum / device_count;
        diagnostics.add_measurement(&BUFFER_FILL_PATH, || average_fill);
    }

    diagnostics.add_measurement(&FRAME_POSITION_PATH, || max_frame);
}
