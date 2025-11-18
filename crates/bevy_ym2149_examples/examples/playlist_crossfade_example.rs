use bevy::prelude::*;
use bevy_ym2149::events::PlaylistAdvanceRequest;
use bevy_ym2149::playlist::{
    CrossfadeConfig, CrossfadeTrigger, PlaylistMode, PlaylistSource, Ym2149Playlist,
    Ym2149PlaylistPlayer,
};
use bevy_ym2149::{Ym2149Playback, Ym2149Plugin};
use bevy_ym2149_examples::ASSET_BASE;

const SONGS: &[&str] = &[
    "music/Ashtray.ym",
    "music/Andy Severn - Lop Ears.aks",
    "music/Credits.ym",
    "music/Amazine 2.ym",
    "music/Decade Where's my willy.ym",
    "music/Doclands - Pong Cracktro (YM).aks",
    "music/LethalXcess1.ym",
    "music/Mon Tune 8.ym",
    "music/Scout.ym",
    "music/Synth Dream 5.ym",
    "music/WingsLeveltune3.ym",
    "music/WingsLeveltune5.ym",
    "music/Excellence in Art 2018 - Just add cream.aks",
    "music/Digizak.ym",
    "music/Gritty.ym",
    "music/Iceage (digi).ym",
];

#[derive(Resource)]
struct UiState {
    playlist_entity: Entity,
    selected: usize,
    entries: Vec<DisplaySong>,
    playing_display: usize,
}

#[derive(Clone)]
struct DisplaySong {
    title: String,
    author: String,
    path: String,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            file_path: ASSET_BASE.into(),
            ..default()
        }))
        .add_plugins(Ym2149Plugin::default())
        .add_systems(Startup, setup)
        .add_systems(Update, (handle_input, update_ui))
        .run();
}

fn setup(mut commands: Commands, mut playlists: ResMut<Assets<Ym2149Playlist>>) {
    commands.spawn(Camera2d);

    let entries: Vec<DisplaySong> = SONGS
        .iter()
        .map(|path| {
            let abs = format!("{}/{}", ASSET_BASE, path);
            let meta = load_metadata(&abs);
            DisplaySong {
                title: meta.0,
                author: meta.1,
                path: path.to_string(),
            }
        })
        .collect();

    // Build playlist asset from bundled songs
    let playlist = Ym2149Playlist {
        tracks: SONGS
            .iter()
            .map(|path| PlaylistSource::Asset {
                path: path.to_string(),
            })
            .collect(),
        mode: PlaylistMode::Loop,
    };
    let playlist_handle = playlists.add(playlist);

    // Crossfade: trigger at 80% of the song, fade until end
    let crossfade = CrossfadeConfig {
        trigger: CrossfadeTrigger::SongRatio(0.8),
        window: bevy_ym2149::playlist::CrossfadeWindow::FixedSeconds(5.0),
    };

    let entity = commands
        .spawn((
            Ym2149Playback::default(),
            Ym2149PlaylistPlayer::with_crossfade(playlist_handle, crossfade),
        ))
        .id();

    commands
        .spawn((
            Node {
                width: Val::Px(1200.0),
                position_type: PositionType::Absolute,
                top: Val::Px(12.0),
                left: Val::Px(12.0),
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.08, 0.1, 0.14)),
            BorderColor::all(Color::srgb(0.25, 0.35, 0.45)),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Playlist loading..."),
                TextFont {
                    font_size: 20.0,
                    ..default()
                },
                TextColor(Color::srgb(0.9, 0.95, 1.0)),
                PlaylistText,
            ));
        });

    commands.insert_resource(UiState {
        playlist_entity: entity,
        selected: 0,
        entries,
        playing_display: 0,
    });
}

fn handle_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut ui: ResMut<UiState>,
    mut advance: MessageWriter<PlaylistAdvanceRequest>,
) {
    if keys.just_pressed(KeyCode::ArrowUp) {
        if ui.selected == 0 {
            ui.selected = SONGS.len().saturating_sub(1);
        } else {
            ui.selected -= 1;
        }
    }
    if keys.just_pressed(KeyCode::ArrowDown) {
        ui.selected = (ui.selected + 1) % SONGS.len();
    }
    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter) {
        advance.write(PlaylistAdvanceRequest {
            entity: ui.playlist_entity,
            index: Some(ui.selected),
        });
        ui.playing_display = ui.selected;
    }
}

#[derive(Component)]
struct PlaylistText;

fn update_ui(
    ui: Res<UiState>,
    mut text_query: Query<&mut Text, With<PlaylistText>>,
    playbacks: Query<&Ym2149PlaylistPlayer>,
) {
    let Ok(mut text) = text_query.single_mut() else {
        return;
    };
    let current = playbacks
        .get(ui.playlist_entity)
        .map(|c| c.current_index)
        .unwrap_or(ui.playing_display);

    let mut lines = Vec::new();
    lines.push("ArrowUp/ArrowDown: select • Enter: play (crossfade)".to_string());
    lines.push("".into());
    for (idx, entry) in ui.entries.iter().enumerate() {
        let marker = if idx == current { "*" } else { " " };
        let selector = if idx == ui.selected { ">" } else { " " };
        lines.push(format!(
            "{selector} {marker} {} — {} ({})",
            entry.title, entry.author, entry.path
        ));
    }

    *text = Text::new(lines.join("\n"));
}

fn load_metadata(path: &str) -> (String, String) {
    let data = std::fs::read(path);
    if let Ok(bytes) = data
        && let Ok((player, _summary)) = ym_replayer::load_song(&bytes)
        && let Some(info) = player.info()
    {
        let title = if info.song_name.trim().is_empty() {
            "Unknown title".to_string()
        } else {
            info.song_name.clone()
        };
        let author = if info.author.trim().is_empty() {
            "Unknown author".to_string()
        } else {
            info.author.clone()
        };
        return (title, author);
    }
    ("Unknown title".to_string(), "Unknown author".to_string())
}
