//! Advanced YM2149 feature showcase
//!
//! Demonstrates playlists, music state graph transitions, diagnostics, and
//! audio bridging with mix controls without relying on the visualization UI.

use bevy::diagnostic::DiagnosticsStore;
use bevy::prelude::*;
use bevy_ym2149::audio_bridge::{AudioBridgeMix, AudioBridgeMixes};
use bevy_ym2149::events::{AudioBridgeRequest, MusicStateRequest, PlaylistAdvanceRequest};
use bevy_ym2149::music_state::{MusicStateDefinition, MusicStateGraph};
use bevy_ym2149::playlist::{PlaylistMode, PlaylistSource, Ym2149Playlist, Ym2149PlaylistPlayer};
use bevy_ym2149::{
    AudioBridgeTargets, Ym2149Playback, Ym2149Plugin, Ym2149PluginConfig, Ym2149Settings,
    FRAME_POSITION_PATH,
};

#[derive(Resource)]
struct DemoPlayback(Entity);

#[derive(Resource, Default)]
struct BridgeRequestSent(bool);

#[derive(Component)]
struct BridgeMixLabel;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(Ym2149Plugin::with_config(
            Ym2149PluginConfig::default().visualization(false),
        ))
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
    commands.spawn(Camera2d::default());

    let mut settings = Ym2149Settings::default();
    settings.loop_enabled = true;
    commands.insert_resource(settings);

    let playlist_handle = playlists.add(Ym2149Playlist {
        tracks: vec![
            PlaylistSource::File {
                path: "examples/ND-Toxygene.ym".into(),
            },
            PlaylistSource::File {
                path: "examples/Credits.ym".into(),
            },
            PlaylistSource::File {
                path: "examples/Ashtray.ym".into(),
            },
            PlaylistSource::File {
                path: "examples/Scout.ym".into(),
            },
        ],
        mode: PlaylistMode::Loop,
    });

    let playback_entity = commands
        .spawn((
            Ym2149Playback::default(),
            Ym2149PlaylistPlayer::new(playlist_handle.clone()),
        ))
        .id();

    commands.insert_resource(DemoPlayback(playback_entity));
    commands.insert_resource(BridgeRequestSent::default());

    let mut graph = MusicStateGraph::default();
    graph.set_target(playback_entity);
    graph.insert(
        "title",
        MusicStateDefinition::SourcePath("examples/ND-Toxygene.ym".into()),
    );
    graph.insert(
        "intense",
        MusicStateDefinition::SourcePath("examples/Steps.ym".into()),
    );
    graph.insert("playlist", MusicStateDefinition::Playlist(playlist_handle));
    commands.insert_resource(graph);

    commands.spawn((
        Text::new(
            "Advanced Feature Showcase\n\
             Controls:\n\
             - [Space] Play/Pause\n\
             - [P] Next playlist entry\n\
             - [1] State: title\n\
             - [2] State: intense\n\
             - [3] State: playlist\n\
             - [A/D] Bridge pan\n\
             - [Z/X] Bridge volume (dB)\n",
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

    if keyboard.just_pressed(KeyCode::Space) {
        if player.is_playing() {
            player.pause();
        } else {
            player.play();
        }
    }

    if keyboard.just_pressed(KeyCode::KeyP) {
        playlist_requests.write(PlaylistAdvanceRequest {
            entity: playback.0,
            index: None,
        });
    }

    if keyboard.just_pressed(KeyCode::Digit1) {
        state_requests.write(MusicStateRequest {
            state: "title".into(),
            target: Some(playback.0),
        });
    }
    if keyboard.just_pressed(KeyCode::Digit2) {
        state_requests.write(MusicStateRequest {
            state: "intense".into(),
            target: Some(playback.0),
        });
    }
    if keyboard.just_pressed(KeyCode::Digit3) {
        state_requests.write(MusicStateRequest {
            state: "playlist".into(),
            target: Some(playback.0),
        });
    }

    if keyboard.just_pressed(KeyCode::KeyL) {
        settings.loop_enabled = !settings.loop_enabled;
        info!("Looping: {}", settings.loop_enabled);
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
        mix = mix.with_volume_db(mix.volume_db() + step);
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
