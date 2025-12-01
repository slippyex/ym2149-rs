//! Music state machine for adaptive audio.
//!
//! This module provides a graph-based state machine for switching between
//! different music tracks or playlists based on game events.

use crate::events::MusicStateRequest;
use crate::playback::Ym2149Playback;
use crate::playlist::{Ym2149Playlist, Ym2149PlaylistPlayer, apply_playlist_entry};
use bevy::prelude::*;
use std::collections::HashMap;

/// Definition of a named music state.
#[derive(Clone)]
pub enum MusicStateDefinition {
    /// Switch playback to the given playlist asset.
    Playlist(Handle<Ym2149Playlist>),
    /// Load a YM file from disk.
    SourcePath(String),
    /// Load YM data from memory.
    Bytes(Vec<u8>),
}

/// Graph mapping state names to definitions along with an optional default target entity.
#[derive(Resource, Default)]
pub struct MusicStateGraph {
    target: Option<Entity>,
    states: HashMap<String, MusicStateDefinition>,
}

impl MusicStateGraph {
    /// Assign a default target entity used when requests omit an explicit one.
    pub fn set_target(&mut self, entity: Entity) {
        self.target = Some(entity);
    }

    /// Clear the default target entity.
    pub fn clear_target(&mut self) {
        self.target = None;
    }

    /// Register or replace a state definition.
    pub fn insert(&mut self, name: impl Into<String>, definition: MusicStateDefinition) {
        self.states.insert(name.into(), definition);
    }

    /// Retrieve a state definition by name.
    pub fn get(&self, name: &str) -> Option<&MusicStateDefinition> {
        self.states.get(name)
    }

    /// Access the default target entity, if one is set.
    pub fn target(&self) -> Option<Entity> {
        self.target
    }
}

/// Process queued music state requests, switching the associated playback sources.
pub fn process_music_state_requests(
    mut commands: Commands,
    mut events: MessageReader<MusicStateRequest>,
    graph: Res<MusicStateGraph>,
    mut playbacks: Query<&mut Ym2149Playback>,
    mut playlist_players: Query<&mut Ym2149PlaylistPlayer>,
    playlists: Res<Assets<Ym2149Playlist>>,
    asset_server: Res<AssetServer>,
) {
    for request in events.read() {
        let Some(definition) = graph.get(&request.state) else {
            warn!("Requested music state '{}' not found", request.state);
            continue;
        };

        let target = request.target.or_else(|| graph.target());
        let Some(entity) = target else {
            warn!("Music state '{}' had no target entity", request.state);
            continue;
        };

        let Ok(mut playback) = playbacks.get_mut(entity) else {
            warn!(
                "Music state '{}' target entity missing Ym2149Playback",
                request.state
            );
            continue;
        };

        match definition.clone() {
            MusicStateDefinition::SourcePath(path) => {
                playback.set_source_path(path);
                playback.restart();
                playback.play();
            }
            MusicStateDefinition::Bytes(bytes) => {
                playback.set_source_bytes(bytes);
                playback.restart();
                playback.play();
            }
            MusicStateDefinition::Playlist(handle) => {
                if let Ok(mut controller) = playlist_players.get_mut(entity) {
                    controller.playlist = handle.clone();
                    controller.current_index = 0;
                } else {
                    commands
                        .entity(entity)
                        .insert(Ym2149PlaylistPlayer::new(handle.clone()));
                }

                if let Some(playlist) = playlists.get(&handle) {
                    if let Some(entry) = playlist.tracks.first() {
                        apply_playlist_entry(entry, &mut playback, &asset_server);
                        playback.restart();
                        playback.play();
                    } else {
                        warn!("Playlist for state '{}' had no tracks", request.state);
                    }
                } else {
                    // Asset not yet loaded; the playlist advance system will apply once ready.
                    playback.restart();
                    playback.play();
                }
            }
        }
    }
}
