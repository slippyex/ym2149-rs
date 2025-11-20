//! Advanced YM2149 playback example with visualization
//!
//! This example demonstrates advanced features of the bevy_ym2149 plugin including:
//! - Real-time visualization (oscilloscope, channel display, spectrum analysis)
//! - File drag-and-drop loading
//! - Keyboard-based playback control

use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use bevy::window::FileDragAndDrop;
use bevy_ym2149::{Ym2149Playback, Ym2149Plugin, Ym2149Settings};
use bevy_ym2149_examples::{ASSET_BASE, embedded_asset_plugin};
use bevy_ym2149_viz::{
    Ym2149VizPlugin, create_channel_visualization, create_detailed_channel_display,
    create_oscilloscope, create_status_display,
};

fn main() {
    App::new()
        .add_plugins(embedded_asset_plugin())
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "YM2149 goes Bevy".into(),
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin {
                    file_path: ASSET_BASE.into(),
                    ..default()
                }),
        )
        .add_plugins(Ym2149Plugin::default())
        .add_plugins(Ym2149VizPlugin)
        .add_systems(Startup, setup)
        .add_systems(Update, (handle_file_drop, playback_controls))
        .run();
}

/// Set up the initial scene with a YM2149 playback entity and visualization
fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    // Spawn a camera for the window
    commands.spawn(Camera2d);

    // Create title/instructions display (positioned below top panel)
    commands.spawn((
        Text::new(
            "Controls:\n\
             - Drag & Drop: Load YM/AKS file\n\
             - SPACE: Play/Pause\n\
             - R: Restart\n\
             - L: Toggle Looping\n\
             - UP/DOWN: Volume Control",
        ),
        TextFont {
            font_size: 18.0,
            ..default()
        },
        TextColor(Color::srgb(0.85, 0.88, 0.94)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(130.0),
            left: Val::Px(10.0),
            ..default()
        },
    ));

    // Create a playback entity
    // Load the default song via Bevy's asset system
    let asset_handle = asset_server.load("music/ND-Toxygene.ym");
    let playback = Ym2149Playback::from_asset(asset_handle);
    commands.spawn(playback);

    // Create detailed channel information display
    create_detailed_channel_display(&mut commands);

    // Create oscilloscope display
    create_oscilloscope(&mut commands);

    // Create channel visualizations (3 channels for YM2149)
    let _channels = create_channel_visualization(&mut commands, 3);

    // Create top panel with song info and status display
    create_status_display(&mut commands);
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
            info!(
                "Looping {}",
                if settings.loop_enabled {
                    "enabled"
                } else {
                    "disabled"
                }
            );
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
    }
}

fn handle_file_drop(
    mut drop_events: MessageReader<FileDragAndDrop>,
    mut playbacks: Query<&mut Ym2149Playback>,
) {
    for event in drop_events.read() {
        if let FileDragAndDrop::DroppedFile { path_buf, .. } = event {
            let path_str = path_buf.to_string_lossy().to_string();
            let lower = path_str.to_lowercase();
            if !lower.ends_with(".ym") && !lower.ends_with(".aks") {
                warn!("Dropped file is not a YM/AKS file: {}", path_str);
                continue;
            }

            if let Some(mut playback) = playbacks.iter_mut().next() {
                playback.set_source_path(path_str.clone());
                playback.play();
                info!("Loaded song from drag-and-drop: {}", path_str);
            }
        }
    }
}
