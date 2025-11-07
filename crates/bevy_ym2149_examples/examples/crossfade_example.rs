//! Dedicated crossfade demonstration.
//!
//! This example wires two YM tracks into a playlist and enables the new
//! `CrossfadeConfig` so that every 15 seconds the upcoming deck preloads and
//! blends in while the current deck fades out. The playlist loops indefinitely
//! which mirrors a simple DJ two-deck workflow.

use bevy::log::info;
use bevy::prelude::*;
use bevy_ym2149::events::TrackStarted;
use bevy_ym2149::playlist::{
    CrossfadeConfig, PlaylistMode, PlaylistSource, Ym2149Playlist, Ym2149PlaylistPlayer,
};
use bevy_ym2149::{Ym2149AudioSource, Ym2149Playback, Ym2149Plugin, Ym2149PluginConfig};
use bevy_ym2149_examples::example_plugins;

const CROSSFADE_SECONDS: f32 = 15.0;

fn main() {
    App::new()
        .add_plugins(example_plugins())
        .add_plugins(Ym2149Plugin::with_config(
            Ym2149PluginConfig::default().visualization(false),
        ))
        .add_systems(Startup, setup_scene)
        .add_systems(Update, (toggle_playback, log_track_transitions))
        .run();
}

fn setup_scene(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut playlists: ResMut<Assets<Ym2149Playlist>>,
) {
    commands.spawn(Camera2d);

    let playlist = playlists.add(Ym2149Playlist {
        tracks: vec![
            PlaylistSource::Asset {
                path: "music/Ashtray.ym".into(),
            },
            PlaylistSource::Asset {
                path: "music/Credits.ym".into(),
            },
        ],
        mode: PlaylistMode::Loop,
    });
    let ym_handle: Handle<Ym2149AudioSource> = asset_server.load("music/Ashtray.ym");
    let mut playback = Ym2149Playback::from_asset(ym_handle);
    playback.play();

    commands.spawn((
        playback,
        Ym2149PlaylistPlayer::with_crossfade(
            playlist,
            CrossfadeConfig::start_at_seconds(CROSSFADE_SECONDS)
                .with_window_seconds(CROSSFADE_SECONDS),
        ),
    ));

    commands.spawn((
        Text::new(format!(
            "Crossfade Loop Demo\n\
             SPACE: Play/Pause current deck\n\
             Playlist: Ashtray -> Credits (loop)\n\
             Next deck starts blending every {CROSSFADE_SECONDS:.0} seconds\n\
             Overlap window: {CROSSFADE_SECONDS:.0} seconds"
        )),
        TextFont {
            font_size: 24.0,
            ..default()
        },
        TextColor(Color::srgb(0.85, 0.9, 0.96)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        },
    ));

    info!(
        "Crossfade demo ready – every {:.0} seconds a new deck fades in across a {:.0}-second window",
        CROSSFADE_SECONDS,
        CROSSFADE_SECONDS
    );
}

fn toggle_playback(mut playbacks: Query<&mut Ym2149Playback>, keyboard: Res<ButtonInput<KeyCode>>) {
    if !keyboard.just_pressed(KeyCode::Space) {
        return;
    }

    if let Ok(mut playback) = playbacks.single_mut() {
        if playback.is_playing() {
            playback.pause();
            info!("Paused crossfade loop");
        } else {
            playback.play();
            info!("Resumed crossfade loop");
        }
    }
}

fn log_track_transitions(
    mut events: MessageReader<TrackStarted>,
    playbacks: Query<&Ym2149Playback>,
) {
    for event in events.read() {
        if let Ok(playback) = playbacks.get(event.entity) {
            let track_name = if playback.song_title.is_empty() {
                playback.source_path().unwrap_or("(unknown)")
            } else {
                playback.song_title.as_str()
            };
            info!("Deck switched to '{}'; crossfade deck queued", track_name);
        }
    }
}
