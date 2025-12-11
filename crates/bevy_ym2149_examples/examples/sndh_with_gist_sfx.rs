//! SNDH Music + GIST Sound Effects Example
//!
//! This example demonstrates using two separate YM2149 emulators:
//! - One for background SNDH music (via Bevy's Ym2149Playback)
//! - One for GIST sound effects triggered by keyboard input
//!
//! Press keys 1-9 to trigger different GIST sound effects over the music.
//!
//! **Architecture Note:** The SNDH player uses the full bevy_ym2149 plugin,
//! while GIST sounds use a standalone GistPlayer that mixes its output
//! directly. This allows independent control of music and SFX volumes.

use bevy::audio::{AddAudioSource, AudioPlayer, Decodable, Source};
use bevy::prelude::*;
use bevy_ym2149::{PlaybackState, Ym2149Playback, Ym2149Plugin};
use bevy_ym2149_examples::{embedded_asset_plugin, example_plugins};
use std::sync::{Arc, Mutex};
use ym2149_gist_replayer::{GistPlayer, GistSound};

/// Resource holding preloaded GIST sound effects
#[derive(Resource)]
struct GistSounds {
    sounds: Vec<(String, GistSound)>,
}

/// Resource for the GIST player (wrapped in Arc<Mutex> for audio thread access)
#[derive(Resource, Clone)]
struct GistPlayerResource(Arc<Mutex<GistPlayer>>);

/// Audio source that generates samples from the GIST player
#[derive(Asset, TypePath, Clone)]
struct GistAudioSource {
    player: Arc<Mutex<GistPlayer>>,
    sample_rate: u32,
}

impl Decodable for GistAudioSource {
    type DecoderItem = <GistDecoder as Iterator>::Item;
    type Decoder = GistDecoder;

    fn decoder(&self) -> Self::Decoder {
        GistDecoder {
            player: Arc::clone(&self.player),
            sample_rate: self.sample_rate,
        }
    }
}

/// Decoder that continuously generates samples from the GIST player
struct GistDecoder {
    player: Arc<Mutex<GistPlayer>>,
    sample_rate: u32,
}

impl Iterator for GistDecoder {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let mut player = self.player.lock().unwrap();
        let samples = player.generate_samples(1);
        Some(samples[0])
    }
}

impl Source for GistDecoder {
    fn current_frame_len(&self) -> Option<usize> {
        None // Infinite stream
    }

    fn channels(&self) -> u16 {
        1 // Mono
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        None // Infinite
    }

    fn try_seek(&mut self, _pos: std::time::Duration) -> Result<(), bevy::audio::SeekError> {
        Ok(()) // No-op for SFX stream
    }
}

fn main() {
    App::new()
        .add_plugins(embedded_asset_plugin())
        .add_plugins(example_plugins())
        .add_plugins(Ym2149Plugin::default())
        .init_asset::<GistAudioSource>()
        .add_audio_source::<GistAudioSource>()
        .add_systems(Startup, setup)
        .add_systems(Update, (handle_input, update_ui))
        .run();
}

fn setup(
    mut commands: Commands,
    mut assets: ResMut<Assets<GistAudioSource>>,
    asset_server: Res<AssetServer>,
) {
    // Spawn camera
    commands.spawn(Camera2d);

    // Load SNDH music via Ym2149Playback
    let asset_handle = asset_server.load("music/Wings_Of_Death.sndh");
    commands.spawn(Ym2149Playback::from_asset(asset_handle));

    // Create the GIST player
    let gist_player = Arc::new(Mutex::new(GistPlayer::new()));
    commands.insert_resource(GistPlayerResource(Arc::clone(&gist_player)));

    // Create audio source for GIST and spawn an audio player
    let gist_source = GistAudioSource {
        player: Arc::clone(&gist_player),
        sample_rate: 44100,
    };
    let gist_handle = assets.add(gist_source);
    commands.spawn(AudioPlayer(gist_handle));

    // Load GIST sound effects from assets
    let gist_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/sfx/gist");
    let sound_files = [
        "laser.snd",
        "explode.snd",
        "shot.snd",
        "alien.snd",
        "gunshot.snd",
        "flyby.snd",
        "whip.snd",
        "sfx1.snd",
        "sfx2.snd",
    ];

    let mut sounds = Vec::new();
    for name in &sound_files {
        let path = format!("{}/{}", gist_dir, name);
        match GistSound::load(&path) {
            Ok(sound) => {
                sounds.push((name.to_string(), sound));
            }
            Err(e) => {
                warn!("Failed to load GIST sound {}: {}", name, e);
            }
        }
    }
    commands.insert_resource(GistSounds { sounds });

    // UI instructions
    commands.spawn((
        Text::new(
            "SNDH + GIST SFX Demo\n\n\
             Background: SNDH music playing\n\
             Press 1-9 to trigger GIST sound effects\n\n\
             SPACE: Pause/Resume music\n\
             R: Restart music\n\
             UP/DOWN: Music volume\n\n\
             Loaded SFX:",
        ),
        TextFont {
            font_size: 20.0,
            ..default()
        },
        TextColor(Color::srgb(0.9, 0.9, 0.9)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        },
        MainText,
    ));
}

#[derive(Component)]
struct MainText;

fn update_ui(
    gist_sounds: Option<Res<GistSounds>>,
    mut text_query: Query<&mut Text, With<MainText>>,
) {
    if let (Some(sounds), Ok(mut text)) = (gist_sounds, text_query.single_mut()) {
        let base = "SNDH + GIST SFX Demo\n\n\
             Background: SNDH music playing\n\
             Press 1-9 to trigger GIST sound effects\n\n\
             SPACE: Pause/Resume music\n\
             R: Restart music\n\
             UP/DOWN: Music volume\n\n\
             Loaded SFX:";

        let mut full_text = base.to_string();
        for (i, (name, _)) in sounds.sounds.iter().enumerate() {
            full_text.push_str(&format!("\n  {}: {}", i + 1, name));
        }

        if text.0 != full_text {
            text.0 = full_text;
        }
    }
}

fn handle_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut playbacks: Query<&mut Ym2149Playback>,
    gist_player: Option<Res<GistPlayerResource>>,
    gist_sounds: Option<Res<GistSounds>>,
) {
    // Music controls
    if let Some(mut playback) = playbacks.iter_mut().next() {
        if keyboard.just_pressed(KeyCode::Space) {
            let state = playback.state;
            if state == PlaybackState::Paused {
                playback.play();
                info!("Music resumed");
            } else if state == PlaybackState::Playing {
                playback.pause();
                info!("Music paused");
            } else {
                playback.restart();
                playback.play();
                info!("Music started");
            }
        }

        if keyboard.just_pressed(KeyCode::KeyR) {
            playback.restart();
            playback.play();
            info!("Music restarted");
        }

        if keyboard.just_pressed(KeyCode::ArrowUp) {
            let new_vol = (playback.volume + 0.1).min(1.0);
            playback.set_volume(new_vol);
            info!("Music volume: {:.0}%", new_vol * 100.0);
        }

        if keyboard.just_pressed(KeyCode::ArrowDown) {
            let new_vol = (playback.volume - 0.1).max(0.0);
            playback.set_volume(new_vol);
            info!("Music volume: {:.0}%", new_vol * 100.0);
        }
    }

    // GIST SFX triggers (keys 1-9)
    if let (Some(player_res), Some(sounds)) = (gist_player, gist_sounds) {
        let key_map = [
            (KeyCode::Digit1, 0),
            (KeyCode::Digit2, 1),
            (KeyCode::Digit3, 2),
            (KeyCode::Digit4, 3),
            (KeyCode::Digit5, 4),
            (KeyCode::Digit6, 5),
            (KeyCode::Digit7, 6),
            (KeyCode::Digit8, 7),
            (KeyCode::Digit9, 8),
        ];

        for (key, index) in key_map {
            if keyboard.just_pressed(key) {
                if let Some((name, sound)) = sounds.sounds.get(index) {
                    if let Ok(mut player) = player_res.0.lock() {
                        player.play_sound(sound, None, None);
                        info!("Playing SFX: {}", name);
                    }
                }
            }
        }
    }
}
