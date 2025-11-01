use crate::events::{PlaylistAdvanceRequest, TrackFinished};
use crate::playback::Ym2149Playback;
use bevy::asset::{io::Reader, AssetLoader, LoadContext};
use bevy::prelude::*;
use bevy::reflect::TypePath;
use serde::Deserialize;

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
}

impl Ym2149PlaylistPlayer {
    pub fn new(playlist: Handle<Ym2149Playlist>) -> Self {
        Self {
            playlist,
            current_index: 0,
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
        if let Ok((mut playback, mut controller)) = players.get_mut(event.entity) {
            if let Some(playlist_asset) = playlists.get(&controller.playlist) {
                if playlist_asset.is_empty() {
                    continue;
                }

                controller.current_index += 1;
                if controller.current_index >= playlist_asset.tracks.len() {
                    match playlist_asset.mode {
                        PlaylistMode::Loop => controller.current_index = 0,
                        PlaylistMode::Once => {
                            controller.current_index =
                                playlist_asset.tracks.len().saturating_sub(1);
                            continue;
                        }
                    }
                }

                if let Some(entry) = playlist_asset.tracks.get(controller.current_index) {
                    apply_playlist_entry(entry, &mut playback, &asset_server);
                    playback.restart();
                    playback.play();
                }
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

        controller.current_index = target_index;

        if let Some(entry) = playlist_asset.tracks.get(target_index) {
            apply_playlist_entry(entry, &mut playback, &asset_server);
            playback.restart();
            playback.play();
        }
    }
}
