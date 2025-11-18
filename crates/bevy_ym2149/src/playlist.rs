use crate::audio_source::Ym2149AudioSource;
use crate::events::{PlaylistAdvanceRequest, TrackFinished};
use crate::playback::{CrossfadeRequest, TrackSource, YM2149_SAMPLE_RATE_F32, Ym2149Playback};
use bevy::asset::{AssetLoader, LoadContext, io::Reader};
use bevy::prelude::*;
use bevy::reflect::TypePath;
use serde::Deserialize;
use std::sync::Arc;

const PLAYLIST_EXTENSIONS: &[&str] = &["ymplaylist", "ympl", "ymlist"];

/// Behaviour when the playlist reaches the last entry.
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlaylistMode {
    #[default]
    Loop,
    Once,
}

/// A single playlist entry.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlaylistSource {
    /// Play a YM file from a filesystem path.
    File { path: String },
    /// Play a `Ym2149AudioSource` asset registered with Bevy's asset server.
    Asset { path: String },
    /// Play YM data embedded directly in the playlist.
    Bytes { data: Vec<u8> },
}

/// Configuration for seamless playlist crossfades.
#[derive(Debug, Clone)]
pub struct CrossfadeConfig {
    pub trigger: CrossfadeTrigger,
    pub window: CrossfadeWindow,
}

impl CrossfadeConfig {
    /// Start the crossfade once the given ratio of the song has elapsed (0.0 - 1.0).
    pub fn start_at_ratio(ratio: f32) -> Self {
        Self {
            trigger: CrossfadeTrigger::SongRatio(ratio),
            window: CrossfadeWindow::UntilSongEnd,
        }
    }

    /// Start the crossfade after a fixed amount of seconds from the beginning of the track.
    pub fn start_at_seconds(seconds: f32) -> Self {
        Self {
            trigger: CrossfadeTrigger::Seconds(seconds),
            window: CrossfadeWindow::UntilSongEnd,
        }
    }

    /// Override the amount of time both decks overlap once the fade begins.
    pub fn with_window_seconds(mut self, seconds: f32) -> Self {
        self.window = CrossfadeWindow::FixedSeconds(seconds.max(0.001));
        self
    }
}

impl Default for CrossfadeConfig {
    fn default() -> Self {
        Self::start_at_ratio(0.9)
    }
}

/// Trigger used to decide when to begin the hand-off to the next deck.
#[derive(Debug, Clone, Copy)]
pub enum CrossfadeTrigger {
    SongRatio(f32),
    Seconds(f32),
}

/// Duration of the overlap between decks once a fade starts.
#[derive(Debug, Clone, Copy)]
pub enum CrossfadeWindow {
    UntilSongEnd,
    FixedSeconds(f32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum CrossfadeStage {
    #[default]
    Idle,
    Loading {
        target_index: usize,
    },
    Active {
        target_index: usize,
    },
}

impl CrossfadeStage {
    fn is_active(&self) -> bool {
        matches!(self, CrossfadeStage::Active { .. })
    }
}

/// Playlist asset describing a set of YM tracks.
#[derive(Asset, Clone, TypePath, Deserialize)]
pub struct Ym2149Playlist {
    pub tracks: Vec<PlaylistSource>,
    #[serde(default)]
    pub mode: PlaylistMode,
}

impl Ym2149Playlist {
    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }
}

/// Loader for `.ymplaylist` assets.
#[derive(Default)]
pub struct Ym2149PlaylistLoader;

impl AssetLoader for Ym2149PlaylistLoader {
    type Asset = Ym2149Playlist;
    type Settings = ();
    type Error = anyhow::Error;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let playlist: Ym2149Playlist = ron::de::from_bytes(&bytes)?;
        Ok(playlist)
    }

    fn extensions(&self) -> &[&str] {
        PLAYLIST_EXTENSIONS
    }
}

/// Component that drives a `Ym2149Playback` using a playlist asset.
#[derive(Component)]
pub struct Ym2149PlaylistPlayer {
    pub playlist: Handle<Ym2149Playlist>,
    pub current_index: usize,
    /// Optional crossfade configuration enabling seamless transitions.
    pub crossfade: Option<CrossfadeConfig>,
    pub(crate) crossfade_stage: CrossfadeStage,
}

impl Ym2149PlaylistPlayer {
    pub fn new(playlist: Handle<Ym2149Playlist>) -> Self {
        Self {
            playlist,
            current_index: 0,
            crossfade: None,
            crossfade_stage: CrossfadeStage::Idle,
        }
    }

    pub fn with_crossfade(playlist: Handle<Ym2149Playlist>, config: CrossfadeConfig) -> Self {
        Self {
            playlist,
            current_index: 0,
            crossfade: Some(config),
            crossfade_stage: CrossfadeStage::Idle,
        }
    }
}

pub fn register_playlist_assets(app: &mut App) {
    app.init_asset_loader::<Ym2149PlaylistLoader>();
}

/// Respond to finished tracks by advancing the playlist and loading the next entry.
pub fn advance_playlist_players(
    mut finished: MessageReader<TrackFinished>,
    mut players: Query<(&mut Ym2149Playback, &mut Ym2149PlaylistPlayer)>,
    playlists: Res<Assets<Ym2149Playlist>>,
    asset_server: Res<AssetServer>,
) {
    for event in finished.read() {
        if let Ok((mut playback, mut controller)) = players.get_mut(event.entity)
            && let Some(playlist_asset) = playlists.get(&controller.playlist)
        {
            if playlist_asset.is_empty() {
                continue;
            }

            if controller.crossfade.is_some()
                && (controller.crossfade_stage.is_active()
                    || playback.is_crossfade_pending()
                    || playback.has_pending_playlist_index())
            {
                continue;
            }

            let Some(next_index) = next_playlist_index(controller.current_index, playlist_asset)
            else {
                continue;
            };

            controller.current_index = next_index;
            controller.crossfade_stage = CrossfadeStage::Idle;
            playback.clear_crossfade_request();

            if let Some(entry) = playlist_asset.tracks.get(controller.current_index) {
                apply_playlist_entry(entry, &mut playback, &asset_server);
                playback.restart();
                playback.play();
            }
        }
    }
}

pub(crate) fn apply_playlist_entry(
    entry: &PlaylistSource,
    playback: &mut Ym2149Playback,
    asset_server: &AssetServer,
) {
    match entry {
        PlaylistSource::File { path } => playback.set_source_path(path.clone()),
        PlaylistSource::Asset { path } => {
            let handle: Handle<crate::audio_source::Ym2149AudioSource> = asset_server.load(path);
            playback.set_source_asset(handle);
        }
        PlaylistSource::Bytes { data } => playback.set_source_bytes(data.clone()),
    }
}

/// Process explicit playlist advance requests (e.g. from UI input).
pub fn handle_playlist_requests(
    mut commands: Commands,
    mut requests: MessageReader<PlaylistAdvanceRequest>,
    mut players: Query<(&mut Ym2149Playback, &mut Ym2149PlaylistPlayer)>,
    playlists: Res<Assets<Ym2149Playlist>>,
    asset_server: Res<AssetServer>,
) {
    for request in requests.read() {
        let Ok((mut playback, mut controller)) = players.get_mut(request.entity) else {
            warn!(
                "Playlist advance request for entity {:?} without controller",
                request.entity
            );
            continue;
        };

        let Some(playlist_asset) = playlists.get(&controller.playlist) else {
            warn!(
                "Playlist asset for entity {:?} not yet loaded; skipping advance",
                request.entity
            );
            continue;
        };

        if playlist_asset.is_empty() {
            continue;
        }

        let mut target_index = request
            .index
            .unwrap_or_else(|| controller.current_index + 1);

        if target_index >= playlist_asset.tracks.len() {
            match playlist_asset.mode {
                PlaylistMode::Loop => target_index %= playlist_asset.tracks.len(),
                PlaylistMode::Once => target_index = playlist_asset.tracks.len() - 1,
            }
        }

        // If nothing is loaded yet, load immediately (first play).
        if playback.player.is_none() {
            controller.current_index = target_index;
            controller.crossfade_stage = CrossfadeStage::Idle;
            if let Some(entry) = playlist_asset.tracks.get(target_index) {
                apply_playlist_entry(entry, &mut playback, &asset_server);
                playback.restart();
                playback.play();
            }
            continue;
        }

        if let Some(cfg) = controller.crossfade.clone() {
            // Cancel any pending/active crossfade and enqueue a fresh one.
            if let Some(cf) = playback.crossfade.take()
                && let Some(cf_entity) = cf.crossfade_entity
            {
                commands.entity(cf_entity).despawn();
            }
            playback.clear_crossfade_request();
            playback.pending_playlist_index = None;

            if let Some(entry) = playlist_asset.tracks.get(target_index) {
                let source = resolve_track_source(entry, &asset_server);
                let desired = match cfg.window {
                    CrossfadeWindow::FixedSeconds(sec) => sec,
                    CrossfadeWindow::UntilSongEnd => {
                        if let Some(metrics) = playback.metrics() {
                            let elapsed = frames_to_seconds(
                                playback.frame_position,
                                metrics.samples_per_frame,
                            );
                            (metrics.duration_seconds() - elapsed).max(0.1)
                        } else {
                            5.0
                        }
                    }
                };
                playback.set_crossfade_request(CrossfadeRequest {
                    source,
                    duration: desired.max(0.1),
                    target_index,
                });
                controller.crossfade_stage = CrossfadeStage::Loading { target_index };
                continue;
            }
        }

        controller.current_index = target_index;
        controller.crossfade_stage = CrossfadeStage::Idle;
        playback.clear_crossfade_request();
        playback.pending_playlist_index = None;

        if let Some(entry) = playlist_asset.tracks.get(target_index) {
            apply_playlist_entry(entry, &mut playback, &asset_server);
            playback.restart();
            playback.play();
        }
    }
}

/// Drive automatic crossfades for playlist-enabled playbacks.
pub fn drive_crossfade_playlists(
    mut players: Query<(&mut Ym2149Playback, &mut Ym2149PlaylistPlayer)>,
    playlists: Res<Assets<Ym2149Playlist>>,
    asset_server: Res<AssetServer>,
) {
    for (mut playback, mut controller) in players.iter_mut() {
        let Some(config) = controller.crossfade.clone() else {
            continue;
        };

        if let Some(new_index) = playback.take_pending_playlist_index() {
            controller.current_index = new_index;
            controller.crossfade_stage = CrossfadeStage::Idle;
        } else if matches!(controller.crossfade_stage, CrossfadeStage::Loading { .. })
            && !playback.is_crossfade_pending()
        {
            controller.crossfade_stage = CrossfadeStage::Idle;
        }

        if let CrossfadeStage::Loading { target_index } = controller.crossfade_stage
            && playback.is_crossfade_active()
        {
            controller.crossfade_stage = CrossfadeStage::Active { target_index };
        }

        let Some(playlist_asset) = playlists.get(&controller.playlist) else {
            continue;
        };
        if playlist_asset.is_empty() {
            continue;
        }

        if playback.is_crossfade_pending() {
            continue;
        }

        let Some(metrics) = playback.metrics() else {
            continue;
        };

        let duration = metrics.duration_seconds();
        if duration <= f32::EPSILON {
            continue;
        }

        let elapsed = frames_to_seconds(playback.frame_position(), metrics.samples_per_frame);
        let trigger_point = match config.trigger {
            CrossfadeTrigger::SongRatio(ratio) => duration * ratio.clamp(0.0, 0.99),
            CrossfadeTrigger::Seconds(seconds) => seconds.max(0.0).min(duration.max(0.0)),
        };

        if elapsed < trigger_point {
            continue;
        }

        let remaining = (duration - elapsed).max(0.0);
        if remaining <= f32::EPSILON {
            continue;
        }

        let Some(next_index) = next_playlist_index(controller.current_index, playlist_asset) else {
            continue;
        };

        let fade_duration = match config.window {
            CrossfadeWindow::UntilSongEnd => remaining,
            CrossfadeWindow::FixedSeconds(seconds) => seconds,
        }
        .max(0.001);

        let Some(entry) = playlist_asset.tracks.get(next_index) else {
            continue;
        };
        let source = resolve_track_source(entry, &asset_server);

        playback.set_crossfade_request(CrossfadeRequest {
            source,
            duration: fade_duration,
            target_index: next_index,
        });
        controller.crossfade_stage = CrossfadeStage::Loading {
            target_index: next_index,
        };
    }
}

fn next_playlist_index(current: usize, playlist: &Ym2149Playlist) -> Option<usize> {
    if playlist.tracks.is_empty() {
        return None;
    }

    let mut next = current + 1;
    if next >= playlist.tracks.len() {
        match playlist.mode {
            PlaylistMode::Loop => next = 0,
            PlaylistMode::Once => return None,
        }
    }

    Some(next)
}

fn frames_to_seconds(frame: u32, samples_per_frame: u32) -> f32 {
    let samples = (frame as usize).saturating_mul(samples_per_frame as usize);
    samples as f32 / YM2149_SAMPLE_RATE_F32
}

fn resolve_track_source(entry: &PlaylistSource, asset_server: &AssetServer) -> TrackSource {
    match entry {
        PlaylistSource::File { path } => TrackSource::File(path.clone()),
        PlaylistSource::Asset { path } => {
            let handle: Handle<Ym2149AudioSource> = asset_server.load(path);
            TrackSource::Asset(handle)
        }
        PlaylistSource::Bytes { data } => TrackSource::Bytes(Arc::new(data.clone())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::playback::{PlaybackMetrics, PlaybackState};
    use bevy::asset::AssetPlugin;

    #[test]
    fn crossfade_request_is_created_after_threshold() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.world_mut().init_resource::<Assets<Ym2149Playlist>>();

        let playlist_handle = {
            let mut assets = app.world_mut().resource_mut::<Assets<Ym2149Playlist>>();
            assets.add(Ym2149Playlist {
                tracks: vec![
                    PlaylistSource::Bytes { data: vec![0; 16] },
                    PlaylistSource::Bytes { data: vec![1; 16] },
                ],
                mode: PlaylistMode::Loop,
            })
        };

        let playback = Ym2149Playback {
            metrics: Some(PlaybackMetrics {
                frame_count: 1_000,
                samples_per_frame: 882,
            }),
            frame_position: 950,
            state: PlaybackState::Playing,
            ..Default::default()
        };

        let controller = Ym2149PlaylistPlayer {
            playlist: playlist_handle,
            current_index: 0,
            crossfade: Some(CrossfadeConfig::default()),
            crossfade_stage: CrossfadeStage::Idle,
        };

        let entity = app.world_mut().spawn((playback, controller)).id();

        app.add_systems(Update, drive_crossfade_playlists);
        app.update();

        let playback = app.world().entity(entity).get::<Ym2149Playback>().unwrap();
        assert!(
            playback.pending_crossfade.is_some(),
            "crossfade should be queued"
        );
        let request = playback.pending_crossfade.as_ref().unwrap();
        assert_eq!(request.target_index, 1);

        let controller = app
            .world()
            .entity(entity)
            .get::<Ym2149PlaylistPlayer>()
            .unwrap();
        assert!(matches!(
            controller.crossfade_stage,
            CrossfadeStage::Loading { target_index: 1 }
        ));
    }

    #[test]
    fn fixed_window_crossfade_uses_requested_duration() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.world_mut().init_resource::<Assets<Ym2149Playlist>>();

        let playlist_handle = {
            let mut assets = app.world_mut().resource_mut::<Assets<Ym2149Playlist>>();
            assets.add(Ym2149Playlist {
                tracks: vec![
                    PlaylistSource::Bytes { data: vec![0; 16] },
                    PlaylistSource::Bytes { data: vec![1; 16] },
                ],
                mode: PlaylistMode::Loop,
            })
        };

        let playback = Ym2149Playback {
            metrics: Some(PlaybackMetrics {
                frame_count: 1_000,
                samples_per_frame: 882,
            }),
            frame_position: 0,
            state: PlaybackState::Playing,
            ..Default::default()
        };

        let controller = Ym2149PlaylistPlayer {
            playlist: playlist_handle,
            current_index: 0,
            crossfade: Some(CrossfadeConfig::start_at_seconds(0.0).with_window_seconds(15.0)),
            crossfade_stage: CrossfadeStage::Idle,
        };

        let entity = app.world_mut().spawn((playback, controller)).id();

        app.add_systems(Update, drive_crossfade_playlists);
        app.update();

        let playback = app.world().entity(entity).get::<Ym2149Playback>().unwrap();
        let request = playback
            .pending_crossfade
            .as_ref()
            .expect("crossfade with fixed window should be queued");
        assert_eq!(request.target_index, 1);
        assert!(
            (request.duration - 15.0).abs() < f32::EPSILON,
            "expected fixed 15 second window, got {}",
            request.duration
        );
    }

    #[test]
    fn crossfade_completion_updates_playlist_index() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.world_mut().init_resource::<Assets<Ym2149Playlist>>();

        let playlist_handle = {
            let mut assets = app.world_mut().resource_mut::<Assets<Ym2149Playlist>>();
            assets.add(Ym2149Playlist {
                tracks: vec![
                    PlaylistSource::Bytes { data: vec![0; 16] },
                    PlaylistSource::Bytes { data: vec![1; 16] },
                ],
                mode: PlaylistMode::Loop,
            })
        };

        let playback = Ym2149Playback {
            metrics: Some(PlaybackMetrics {
                frame_count: 1_000,
                samples_per_frame: 882,
            }),
            pending_playlist_index: Some(1),
            ..Default::default()
        };

        let controller = Ym2149PlaylistPlayer {
            playlist: playlist_handle,
            current_index: 0,
            crossfade: Some(CrossfadeConfig::default()),
            crossfade_stage: CrossfadeStage::Active { target_index: 1 },
        };

        let entity = app.world_mut().spawn((playback, controller)).id();

        app.add_systems(Update, drive_crossfade_playlists);
        app.update();

        let playback = app.world().entity(entity).get::<Ym2149Playback>().unwrap();
        assert!(playback.pending_playlist_index.is_none());

        let controller = app
            .world()
            .entity(entity)
            .get::<Ym2149PlaylistPlayer>()
            .unwrap();
        assert_eq!(controller.current_index, 1);
        assert!(matches!(controller.crossfade_stage, CrossfadeStage::Idle));
    }

    #[test]
    fn track_finished_ignored_during_crossfade() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.add_message::<TrackFinished>();
        app.world_mut().init_resource::<Assets<Ym2149Playlist>>();

        let playlist_handle = {
            let mut assets = app.world_mut().resource_mut::<Assets<Ym2149Playlist>>();
            assets.add(Ym2149Playlist {
                tracks: vec![
                    PlaylistSource::Bytes { data: vec![0; 16] },
                    PlaylistSource::Bytes { data: vec![1; 16] },
                ],
                mode: PlaylistMode::Loop,
            })
        };

        let playback = Ym2149Playback {
            metrics: Some(PlaybackMetrics {
                frame_count: 1_000,
                samples_per_frame: 882,
            }),
            pending_crossfade: Some(CrossfadeRequest {
                source: TrackSource::Bytes(Arc::new(vec![2; 16])),
                duration: 1.0,
                target_index: 1,
            }),
            ..Default::default()
        };

        let controller = Ym2149PlaylistPlayer {
            playlist: playlist_handle,
            current_index: 0,
            crossfade: Some(CrossfadeConfig::default()),
            crossfade_stage: CrossfadeStage::Active { target_index: 1 },
        };

        let entity = app.world_mut().spawn((playback, controller)).id();

        app.add_systems(Update, advance_playlist_players);

        app.world_mut()
            .resource_mut::<Messages<TrackFinished>>()
            .write(TrackFinished { entity });

        app.update();

        let controller = app
            .world()
            .entity(entity)
            .get::<Ym2149PlaylistPlayer>()
            .unwrap();
        assert_eq!(controller.current_index, 0);
    }
}
