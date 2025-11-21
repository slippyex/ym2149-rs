//! Comprehensive YM2149 feature showcase
//!
//! Demonstrates advanced features including:
//! - Multiple simultaneous YM file playbacks
//! - Playlist management with automatic progression
//! - Music state graphs for dynamic music transitions
//! - Audio bridge mixing with real-time parameter control
//! - Playback diagnostics and frame position tracking
//! - Event-driven architecture for track transitions
//! - Optional visualization integration
//!
//! This example shows how to build a sophisticated music system without relying solely on UI.

use bevy::asset::AssetPlugin;
use bevy::diagnostic::DiagnosticsStore;
use bevy::prelude::*;
use bevy_ym2149::audio_bridge::{AudioBridgeMix, AudioBridgeMixes};
use bevy_ym2149::events::{AudioBridgeRequest, MusicStateRequest, PlaylistAdvanceRequest};
use bevy_ym2149::music_state::{MusicStateDefinition, MusicStateGraph};
use bevy_ym2149::playlist::{PlaylistMode, PlaylistSource, Ym2149Playlist, Ym2149PlaylistPlayer};
use bevy_ym2149::{
    AudioBridgeTargets, FRAME_POSITION_PATH, Ym2149Playback, Ym2149Plugin, Ym2149PluginConfig,
    Ym2149Settings,
};
use bevy_ym2149_examples::{ASSET_BASE, embedded_asset_plugin};

#[derive(Resource)]
struct DemoPlayback(Entity);

#[derive(Resource)]
struct SecondaryPlayback(Entity);

#[derive(Resource, Default)]
struct BridgeRequestSent(bool);

#[derive(Component)]
struct BridgeMixLabel;

fn main() {
    App::new()
        .add_plugins(embedded_asset_plugin())
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            file_path: ASSET_BASE.into(),
            ..default()
        }))
        .add_plugins(Ym2149Plugin::with_config(Ym2149PluginConfig {
            bevy_audio_bridge: true, // Enable audio bridge for this example
            ..Default::default()
        }))
        .add_systems(Startup, setup_demo)
        .add_systems(Update, demo_keyboard_controls)
        .add_systems(Update, request_bridge_audio)
        .add_systems(Update, update_bridge_mix)
        .add_systems(Update, print_diagnostics)
        .run();
}

fn setup_demo(
    mut commands: Commands,
    mut playlists: ResMut<Assets<Ym2149Playlist>>,
    mut mixes: ResMut<AudioBridgeMixes>,
) {
    commands.spawn(Camera2d);

    commands.insert_resource(Ym2149Settings {
        loop_enabled: true,
        ..Default::default()
    });

    let playlist_handle = playlists.add(Ym2149Playlist {
        tracks: vec![
            PlaylistSource::File {
                path: "examples/ym/ND-Toxygene.ym".into(),
            },
            PlaylistSource::File {
                path: "examples/ym/Credits.ym".into(),
            },
            PlaylistSource::File {
                path: "examples/ym/Ashtray.ym".into(),
            },
            PlaylistSource::File {
                path: "examples/ym/Scout.ym".into(),
            },
        ],
        mode: PlaylistMode::Loop,
    });

    // Primary playback with playlist and state transitions
    let playback_entity = commands
        .spawn((
            Ym2149Playback::default(),
            Ym2149PlaylistPlayer::new(playlist_handle.clone()),
        ))
        .id();

    // Secondary playback for simultaneous music playback demonstration
    // This shows that multiple YM2149 players can run independently
    let secondary_entity = commands
        .spawn(Ym2149Playback::new("examples/ym/Scout.ym"))
        .id();

    commands.insert_resource(DemoPlayback(playback_entity));
    commands.insert_resource(SecondaryPlayback(secondary_entity));
    commands.insert_resource(BridgeRequestSent::default());

    let mut graph = MusicStateGraph::default();
    graph.set_target(playback_entity);
    graph.insert(
        "title",
        MusicStateDefinition::SourcePath("examples/ym/ND-Toxygene.ym".into()),
    );
    graph.insert(
        "intense",
        MusicStateDefinition::SourcePath("examples/ym/Steps.ym".into()),
    );
    graph.insert("playlist", MusicStateDefinition::Playlist(playlist_handle));
    commands.insert_resource(graph);

    commands.spawn((
        Text::new(
            "Comprehensive YM2149 Feature Showcase\n\n\
             Primary Playback (Playlist + State Graph):\n\
             - [Space] Play/Pause\n\
             - [P] Next playlist entry\n\
             - [1] State: title\n\
             - [2] State: intense\n\
             - [3] State: playlist\n\
             - [L] Toggle looping\n\n\
             Secondary Playback (Independent):\n\
             - [S] Play/Pause\n\
             - [U/O] Volume control\n\n\
             Audio Bridge Controls:\n\
             - [A/D] Pan (+/- 0.1)\n\
             - [Z/X] Volume (+/- 1 dB)\n",
        ),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        },
    ));

    commands.spawn((
        Text::new("Bridge Mix: 0.0 dB @ 0.00"),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        },
        BridgeMixLabel,
    ));

    mixes.set(playback_entity, AudioBridgeMix::CENTER);
}

fn demo_keyboard_controls(
    playback: Option<Res<DemoPlayback>>,
    secondary: Option<Res<SecondaryPlayback>>,
    mut playbacks: Query<&mut Ym2149Playback>,
    mut settings: ResMut<Ym2149Settings>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut playlist_requests: MessageWriter<PlaylistAdvanceRequest>,
    mut state_requests: MessageWriter<MusicStateRequest>,
) {
    let Some(playback) = playback else { return };
    let Ok(mut player) = playbacks.get_mut(playback.0) else {
        return;
    };

    // Primary playback controls
    if keyboard.just_pressed(KeyCode::Space) {
        if player.is_playing() {
            player.pause();
            info!("Primary playback paused");
        } else {
            player.play();
            info!("Primary playback resumed");
        }
    }

    if keyboard.just_pressed(KeyCode::KeyP) {
        playlist_requests.write(PlaylistAdvanceRequest {
            entity: playback.0,
            index: None,
        });
        info!("Advancing to next playlist entry");
    }

    if keyboard.just_pressed(KeyCode::Digit1) {
        state_requests.write(MusicStateRequest {
            state: "title".into(),
            target: Some(playback.0),
        });
        info!("Transitioning to 'title' state");
    }
    if keyboard.just_pressed(KeyCode::Digit2) {
        state_requests.write(MusicStateRequest {
            state: "intense".into(),
            target: Some(playback.0),
        });
        info!("Transitioning to 'intense' state");
    }
    if keyboard.just_pressed(KeyCode::Digit3) {
        state_requests.write(MusicStateRequest {
            state: "playlist".into(),
            target: Some(playback.0),
        });
        info!("Transitioning to 'playlist' state");
    }

    if keyboard.just_pressed(KeyCode::KeyL) {
        settings.loop_enabled = !settings.loop_enabled;
        info!("Primary playback looping: {}", settings.loop_enabled);
    }

    // Secondary playback controls (independent)
    if let Some(secondary) = secondary
        && let Ok(mut secondary_player) = playbacks.get_mut(secondary.0)
    {
        if keyboard.just_pressed(KeyCode::KeyS) {
            if secondary_player.is_playing() {
                secondary_player.pause();
                info!("Secondary playback paused");
            } else {
                secondary_player.play();
                info!("Secondary playback started");
            }
        }

        if keyboard.just_pressed(KeyCode::KeyU) {
            let new_volume = (secondary_player.volume + 0.1).min(1.0);
            secondary_player.set_volume(new_volume);
            info!("Secondary volume: {:.0}%", new_volume * 100.0);
        }

        if keyboard.just_pressed(KeyCode::KeyO) {
            let new_volume = (secondary_player.volume - 0.1).max(0.0);
            secondary_player.set_volume(new_volume);
            info!("Secondary volume: {:.0}%", new_volume * 100.0);
        }
    }
}

fn request_bridge_audio(
    playback: Option<Res<DemoPlayback>>,
    targets: Option<Res<AudioBridgeTargets>>,
    mut writer: MessageWriter<AudioBridgeRequest>,
    mut sent: ResMut<BridgeRequestSent>,
) {
    if sent.0 {
        return;
    }
    let Some(playback) = playback else { return };
    let requested = targets
        .as_ref()
        .map(|t| t.0.contains(&playback.0))
        .unwrap_or(false);
    if requested {
        sent.0 = true;
        return;
    }
    writer.write(AudioBridgeRequest { entity: playback.0 });
    sent.0 = true;
}

fn update_bridge_mix(
    playback: Option<Res<DemoPlayback>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut mixes: ResMut<AudioBridgeMixes>,
    mut labels: Query<&mut Text, With<BridgeMixLabel>>,
) {
    let Some(playback) = playback else { return };
    let mut mix = mixes.get(playback.0);
    let mut changed = false;

    if keyboard.any_just_pressed([KeyCode::KeyZ, KeyCode::KeyX]) {
        let step = if keyboard.just_pressed(KeyCode::KeyZ) {
            -1.0
        } else {
            1.0
        };
        mix.volume = AudioBridgeMix::db_to_gain(mix.volume_db() + step);
        changed = true;
    }
    if keyboard.any_just_pressed([KeyCode::KeyA, KeyCode::KeyD]) {
        let delta = if keyboard.just_pressed(KeyCode::KeyA) {
            -0.1
        } else {
            0.1
        };
        mix.pan = (mix.pan + delta).clamp(-1.0, 1.0);
        changed = true;
    }

    if changed {
        mixes.set(playback.0, mix);
    }

    if let Some(mut label) = labels.iter_mut().next() {
        label.0 = format!("Bridge Mix: {:+.1} dB @ {:+.2}", mix.volume_db(), mix.pan);
    }
}

fn print_diagnostics(diagnostics: Res<DiagnosticsStore>, time: Res<Time>, mut elapsed: Local<f32>) {
    *elapsed += time.delta_secs();
    if *elapsed >= 5.0 {
        *elapsed = 0.0;
        if let Some(diag) = diagnostics
            .get(&FRAME_POSITION_PATH)
            .and_then(|d| d.value())
        {
            info!("Frame position diagnostic: {:.0}", diag);
        }
    }
}
