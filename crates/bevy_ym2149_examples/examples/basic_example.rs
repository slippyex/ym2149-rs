//! Minimal YM2149 playback example
//!
//! This is the simplest possible example showing how to:
//! - Create a Bevy app with the YM2149 plugin
//! - Load and play a YM file
//! - Control playback with basic keyboard input

use bevy::log::info;
use bevy::prelude::*;
use bevy_ym2149::{Ym2149Playback, Ym2149Plugin, YmSfxRequest};
use bevy_ym2149_examples::example_plugins;

fn main() {
    App::new()
        .add_plugins(example_plugins())
        .add_plugins(Ym2149Plugin::default())
        .add_systems(Startup, setup)
        .add_systems(Update, playback_control)
        .run();
}

/// Set up the initial scene with a YM2149 playback entity
fn setup(mut commands: Commands) {
    // Spawn a camera
    commands.spawn(Camera2d);

    // Spawn a YM2149 playback entity with a file path
    // This uses a bundled example music file; replace with your own YM file path if desired
    commands.spawn(Ym2149Playback::new("examples/Syntax Terror - TLB.ym"));

    // Display simple instructions
    commands.spawn((
        Text::new(
            "YM2149 Player\n\
             SPACE: Play/Pause\n\
             R: Restart\n\
             UP/DOWN: Volume",
        ),
        TextFont {
            font_size: 24.0,
            ..default()
        },
        TextColor(Color::srgb(0.85, 0.88, 0.94)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        },
    ));
}

/// Handle basic keyboard input for playback control
fn playback_control(
    mut playbacks: Query<&mut Ym2149Playback>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut sfx: MessageWriter<YmSfxRequest>,
) {
    if let Some(mut playback) = playbacks.iter_mut().next() {
        // Play/Pause toggle on spacebar
        if keyboard.just_pressed(KeyCode::Space) {
            if playback.is_playing() {
                playback.pause();
                info!("Paused");
            } else {
                playback.play();
                info!("Playing");
            }
        }

        // Restart on 'R'
        if keyboard.just_pressed(KeyCode::KeyR) {
            playback.restart();
            playback.play();
            info!("Restarted");
        }

        // Volume control with arrow keys
        if keyboard.just_pressed(KeyCode::ArrowUp) {
            let new_volume = (playback.volume + 0.1).min(1.0);
            playback.set_volume(new_volume);
            info!("Volume: {:.0}%", new_volume * 100.0);
        }

        if keyboard.just_pressed(KeyCode::ArrowDown) {
            let new_volume = (playback.volume - 0.1).max(0.0);
            playback.set_volume(new_volume);
            info!("Volume: {:.0}%", new_volume * 100.0);
        }

        // Quick SFX demo (PSG one-shots) on Q/W/E
        if keyboard.just_pressed(KeyCode::KeyQ) {
            sfx.write(YmSfxRequest {
                target: None,
                channel: 0,
                freq_hz: 440.0,
                volume: 0.8,
                duration_frames: 12,
            });
        }
        if keyboard.just_pressed(KeyCode::KeyW) {
            sfx.write(YmSfxRequest {
                target: None,
                channel: 1,
                freq_hz: 660.0,
                volume: 0.7,
                duration_frames: 12,
            });
        }
        if keyboard.just_pressed(KeyCode::KeyE) {
            sfx.write(YmSfxRequest {
                target: None,
                channel: 2,
                freq_hz: 880.0,
                volume: 0.6,
                duration_frames: 12,
            });
        }
    }
}
