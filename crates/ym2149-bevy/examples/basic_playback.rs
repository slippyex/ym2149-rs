//! Basic YM2149 playback example with visualization
//!
//! This example demonstrates how to use the ym2149-bevy plugin to load and play
//! YM chiptune files in a Bevy application with real-time visualization.

use bevy::prelude::*;
use ym2149_bevy::{
    create_detailed_channel_display, create_status_display, Ym2149Playback, Ym2149Plugin,
    Ym2149Settings,
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "YM2149 goes Bevy".into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(Ym2149Plugin)
        .add_systems(Startup, setup)
        .add_systems(Update, playback_controls)
        .run();
}

/// Set up the initial scene with a YM2149 playback entity and visualization
fn setup(mut commands: Commands) {
    // Spawn a camera for the window
    commands.spawn(Camera2d::default());

    // Create title/instructions display (positioned below top panel)
    commands.spawn((
        Text::new(
            "Controls:\n\
             - Drag & Drop: Load YM file\n\
             - SPACE: Play/Pause\n\
             - R: Restart\n\
             - L: Toggle Looping\n\
             - UP/DOWN: Volume Control",
        ),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(90.0),
            left: Val::Px(10.0),
            ..default()
        },
    ));

    // Create a playback entity
    // The path will be set via drag-and-drop or manually loaded
    // You can also specify a default file path if desired
    let playback = Ym2149Playback::new("examples/ND-Toxygene.ym");
    // Note: playback starts in Idle state; drag-and-drop or keyboard controls will start it
    commands.spawn(playback);

    // Create top panel with song info and status display
    create_status_display(&mut commands);

    // Create detailed channel information display
    create_detailed_channel_display(&mut commands);

    // Create channel visualizations (3 channels for YM2149)
    let _channels = ym2149_bevy::visualization::create_channel_visualization(&mut commands, 3);
}

/// Handle keyboard input for playback control
fn playback_controls(
    mut playbacks: Query<&mut Ym2149Playback>,
    mut settings: ResMut<Ym2149Settings>,
    keyboard: Res<ButtonInput<KeyCode>>,
) {
    if let Some(mut playback) = playbacks.iter_mut().next() {
        // Play/Pause toggle on spacebar
        if keyboard.just_pressed(KeyCode::Space) {
            if playback.is_playing() {
                playback.pause();
            } else {
                playback.play();
            }
        }

        // Restart on 'R'
        if keyboard.just_pressed(KeyCode::KeyR) {
            playback.restart();
            playback.play();
        }

        // Toggle looping on 'L'
        if keyboard.just_pressed(KeyCode::KeyL) {
            settings.loop_enabled = !settings.loop_enabled;
            let status = if settings.loop_enabled {
                "enabled"
            } else {
                "disabled"
            };
            println!("Looping {}", status);
        }

        // Volume control with arrow keys
        if keyboard.just_pressed(KeyCode::ArrowUp) {
            let new_volume = (playback.volume + 0.1).min(1.0);
            playback.set_volume(new_volume);
            println!("Volume: {:.0}%", new_volume * 100.0);
        }

        if keyboard.just_pressed(KeyCode::ArrowDown) {
            let new_volume = (playback.volume - 0.1).max(0.0);
            playback.set_volume(new_volume);
            println!("Volume: {:.0}%", new_volume * 100.0);
        }
    }
}
