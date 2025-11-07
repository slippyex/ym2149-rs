//! Advanced YM2149 playback example with visualization and audio bridge mixing
//!
//! This example demonstrates advanced features of the bevy_ym2149 plugin including:
//! - Real-time visualization (oscilloscope, channel display, spectrum analysis)
//! - File drag-and-drop loading
//! - Audio bridge mixing with volume and pan controls
//! - Keyboard-based playback control

use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use bevy::window::FileDragAndDrop;
use bevy_ym2149::{
    audio_bridge::{AudioBridgeMix, AudioBridgeMixes},
    AudioBridgeRequest, AudioBridgeTargets, Ym2149Playback, Ym2149Plugin, Ym2149Settings,
};
use bevy_ym2149_examples::ASSET_BASE;
use bevy_ym2149_viz::{
    create_channel_visualization, create_detailed_channel_display, create_oscilloscope,
    create_status_display, Ym2149VizPlugin,
};

#[derive(Resource)]
struct PlaybackEntity(Entity);

#[derive(Component)]
struct BridgeMixLabel;

#[derive(Resource)]
struct BridgeControl {
    enabled: bool,
}

fn main() {
    App::new()
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
        .add_plugins(Ym2149VizPlugin::default())
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (handle_file_drop, playback_controls, bridge_mix_controls),
        )
        .run();
}

/// Set up the initial scene with a YM2149 playback entity and visualization
fn setup(mut commands: Commands) {
    // Spawn a camera for the window
    commands.spawn(Camera2d);

    // Create title/instructions display (positioned below top panel)
    commands.spawn((
        Text::new(
            "Controls:\n\
             - Drag & Drop: Load YM file\n\
             - SPACE: Play/Pause\n\
             - R: Restart\n\
             - L: Toggle Looping\n\
             - UP/DOWN: Volume Control\n\
             - B: Toggle Bridge Audio\n\
             - Z/X: Bridge Volume (+/-1 dB)\n\
             - A/D: Bridge Pan (+/-0.1)",
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
    // The path will be set via drag-and-drop or manually loaded
    // You can also specify a default file path if desired
    let playback = Ym2149Playback::new("examples/ND-Toxygene.ym");
    let playback_entity = commands.spawn(playback).id();
    commands.insert_resource(PlaybackEntity(playback_entity));
    commands.insert_resource(BridgeControl { enabled: false });

    // Create detailed channel information display
    create_detailed_channel_display(&mut commands);

    // Create oscilloscope display
    create_oscilloscope(&mut commands);

    // Create channel visualizations (3 channels for YM2149)
    let _channels = create_channel_visualization(&mut commands, 3);

    // Create top panel with song info and status display
    create_status_display(&mut commands);

    // Display bridge mix information and controls hint
    commands.spawn((
        Text::new("Bridge Audio: disabled\nVolume: --.- dB\nPan: --.--\n[B] Toggle"),
        TextFont {
            font_size: 18.0,
            ..default()
        },
        TextColor(Color::srgb(0.85, 0.88, 0.94)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(360.0),
            left: Val::Px(10.0),
            ..default()
        },
        BridgeMixLabel,
    ));
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
            if !path_str.to_lowercase().ends_with(".ym") {
                warn!("Dropped file is not a YM file: {}", path_str);
                continue;
            }

            if let Some(mut playback) = playbacks.iter_mut().next() {
                playback.set_source_path(path_str.clone());
                playback.play();
                info!("Loaded YM file from drag-and-drop: {}", path_str);
            }
        }
    }
}

fn bridge_mix_controls(
    playback: Option<Res<PlaybackEntity>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut mixes: ResMut<AudioBridgeMixes>,
    mut targets: ResMut<AudioBridgeTargets>,
    mut control: ResMut<BridgeControl>,
    mut requests: MessageWriter<AudioBridgeRequest>,
    mut labels: Query<&mut Text, With<BridgeMixLabel>>,
) {
    let Some(playback) = playback else { return };
    let mut mix = mixes.get(playback.0);
    let mut changed = false;

    if keyboard.just_pressed(KeyCode::KeyB) {
        control.enabled = !control.enabled;
        if control.enabled {
            mixes.set(playback.0, AudioBridgeMix::CENTER);
            mix = AudioBridgeMix::CENTER;
            requests.write(AudioBridgeRequest { entity: playback.0 });
        } else {
            targets.0.remove(&playback.0);
        }
        changed = true; // Update label when toggling
    }

    if control.enabled {
        if keyboard.any_just_pressed([KeyCode::KeyZ, KeyCode::KeyX]) {
            let step_db = if keyboard.just_pressed(KeyCode::KeyZ) {
                -1.0
            } else {
                1.0
            };
            mix = mix.with_volume_db(mix.volume_db() + step_db);
            changed = true;
        }

        if keyboard.any_just_pressed([KeyCode::KeyA, KeyCode::KeyD]) {
            let delta = if keyboard.just_pressed(KeyCode::KeyA) {
                -0.1
            } else {
                0.1
            };
            mix = mix.with_pan((mix.pan + delta).clamp(-1.0, 1.0));
            changed = true;
        }
    }

    if changed {
        if control.enabled {
            mixes.set(playback.0, mix);
        }

        // Update label whenever something changes
        if let Some(mut label) = labels.iter_mut().next() {
            if control.enabled {
                label.0 = format!(
                    "Bridge Audio: enabled\nVolume: {:+.1} dB\nPan: {:+.2}\n[B] Toggle",
                    mix.volume_db(),
                    mix.pan
                );
            } else {
                label.0 =
                    "Bridge Audio: disabled\nVolume: --.- dB\nPan: --.--\n[B] Toggle".to_string();
            }
        }
    }
}
