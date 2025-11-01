use bevy::asset::AssetPlugin;
use bevy::diagnostic::{DiagnosticsPlugin, DiagnosticsStore};
use bevy::prelude::Messages;
use bevy::prelude::*;
use bevy_ym2149::{
    advance_playlist_players, process_music_state_requests, update_diagnostics,
    MusicStateDefinition, MusicStateGraph, MusicStateRequest, PlaylistMode, PlaylistSource,
    TrackFinished, Ym2149Playback, Ym2149Playlist, Ym2149PlaylistPlayer, Ym2149PluginConfig,
    FRAME_POSITION_PATH,
};

#[test]
fn playlist_advances_to_next_entry() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()));
    app.add_message::<TrackFinished>();
    app.add_systems(Update, advance_playlist_players);
    app.world_mut().init_resource::<Assets<Ym2149Playlist>>();

    let handle = {
        let mut assets = app.world_mut().resource_mut::<Assets<Ym2149Playlist>>();
        assets.add(Ym2149Playlist {
            tracks: vec![
                PlaylistSource::Bytes { data: vec![0; 64] },
                PlaylistSource::Bytes { data: vec![1; 64] },
            ],
            mode: PlaylistMode::Loop,
        })
    };

    let entity = app
        .world_mut()
        .spawn((Ym2149Playback::default(), Ym2149PlaylistPlayer::new(handle)))
        .id();

    app.world_mut()
        .resource_mut::<Messages<TrackFinished>>()
        .write(TrackFinished { entity });

    app.update();

    let playback = app.world().entity(entity).get::<Ym2149Playback>().unwrap();
    assert!(
        playback.source_bytes().is_some(),
        "playlist should set source bytes"
    );
    assert_eq!(playback.state, bevy_ym2149::PlaybackState::Playing);
}

#[test]
fn music_state_request_switches_source_path() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()));
    app.add_message::<MusicStateRequest>();
    app.insert_resource(MusicStateGraph::default());
    app.world_mut().init_resource::<Assets<Ym2149Playlist>>();
    app.add_systems(Update, process_music_state_requests);

    let entity = app.world_mut().spawn(Ym2149Playback::default()).id();

    {
        let mut graph = app.world_mut().resource_mut::<MusicStateGraph>();
        graph.set_target(entity);
        graph.insert(
            "battle",
            MusicStateDefinition::SourcePath("music/battle.ym".into()),
        );
    }

    app.world_mut()
        .resource_mut::<Messages<MusicStateRequest>>()
        .write(MusicStateRequest {
            state: "battle".into(),
            target: None,
        });

    app.update();

    let playback = app.world().entity(entity).get::<Ym2149Playback>().unwrap();
    assert_eq!(playback.source_path(), Some("music/battle.ym"));
    assert_eq!(playback.state, bevy_ym2149::PlaybackState::Playing);
}

#[test]
fn diagnostics_record_frame_position() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, DiagnosticsPlugin::default()));
    app.insert_resource(Ym2149PluginConfig::default());
    bevy_ym2149::diagnostics::register(&mut app);
    app.add_systems(Update, update_diagnostics);

    let entity = app.world_mut().spawn(Ym2149Playback::default()).id();

    app.world_mut()
        .entity_mut(entity)
        .get_mut::<Ym2149Playback>()
        .unwrap()
        .frame_position = 128;

    app.update();

    let store = app.world().resource::<DiagnosticsStore>();
    let diagnostic = store
        .get(&FRAME_POSITION_PATH)
        .and_then(|diag| diag.value());
    assert_eq!(diagnostic, Some(128.0));
}
