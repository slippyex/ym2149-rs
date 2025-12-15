//! Space Shooter - A Galaxian-style retro game with CRT effect
//! Controls: Arrows (move), Space (fire), Enter (start), R (restart), M (music), C (CRT toggle), Q (quit)
//! CLI: -m false (disable music)

mod space_shooter {
    pub mod audio;
    pub mod components;
    pub mod config;
    pub mod crt;
    pub mod resources;
    pub mod spawning;
    pub mod systems;
    pub mod ui;
}

use bevy::audio::{AddAudioSource, AudioPlayer};
use bevy::camera::{ClearColorConfig, RenderTarget};
use bevy::math::primitives::Rectangle;
use bevy::prelude::*;
use bevy::render::render_resource::{
    Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};
use bevy::sprite_render::{Material2dPlugin, MeshMaterial2d};
use bevy::window::PrimaryWindow;
use bevy_mesh::Mesh2d;
use bevy_ym2149::{Ym2149Playback, Ym2149Plugin};
use bevy_ym2149_examples::{embedded_asset_plugin, example_plugins};
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use std::env;
use std::sync::{Arc, Mutex};
use ym2149_gist_replayer::{GistPlayer, GistSound};

use bevy::ecs::system::SystemParam;

use space_shooter::{
    audio::GistAudio,
    components::*,
    config::*,
    crt::{CrtMaterial, CrtParams, crt_toggle, sync_render_target, update_crt_material},
    resources::*,
    systems::*,
    ui::*,
};

/// Asset managers for setup
#[derive(SystemParam)]
struct SetupAssets<'w> {
    meshes: ResMut<'w, Assets<Mesh>>,
    images: ResMut<'w, Assets<Image>>,
    crt_materials: ResMut<'w, Assets<CrtMaterial>>,
    atlas_layouts: ResMut<'w, Assets<TextureAtlasLayout>>,
}

fn parse_args() -> bool {
    let args: Vec<String> = env::args().collect();
    let mut music_enabled = true;

    let mut i = 1;
    while i < args.len() {
        if args[i] == "-m" && i + 1 < args.len() {
            music_enabled = args[i + 1].to_lowercase() != "false";
            i += 2;
        } else {
            i += 1;
        }
    }
    music_enabled
}

fn main() {
    let music_enabled = parse_args();

    App::new()
        .insert_resource(MusicEnabled(music_enabled))
        .add_plugins((
            embedded_asset_plugin(),
            example_plugins(),
            Ym2149Plugin::default(),
            Material2dPlugin::<CrtMaterial>::default(),
        ))
        .init_asset::<GistAudio>()
        .add_audio_source::<GistAudio>()
        .init_state::<GameState>()
        .insert_resource(GameData::new())
        .insert_resource(CrtState { enabled: true })
        .insert_resource(MusicFade::default())
        .insert_resource(HighScoreList::load())
        .insert_resource(NameEntryState::default())
        .insert_resource(AttractMode::default())
        .insert_resource(PowerUpState::default())
        .insert_resource(QuitConfirmation::default())
        .insert_resource(NewHighScoreIndex::default())
        .insert_resource(ScreenCycleTimer::default())
        .insert_resource(ScreenFade::default())
        .insert_resource(ScreenShake::default())
        .insert_resource(ComboTracker::default())
        .insert_resource(ScreenFlash::default())
        .add_message::<PlaySfxMsg>()
        .add_message::<WaveCompleteMsg>()
        .add_message::<PlayerHitMsg>()
        .add_message::<EnemyKilledMsg>()
        .add_message::<ExtraLifeMsg>()
        .add_message::<PowerUpCollectedMsg>()
        .configure_sets(
            Update,
            (
                GameSet::Input,
                GameSet::Movement,
                GameSet::Collision,
                GameSet::Spawn,
                GameSet::Cleanup,
            )
                .chain(),
        )
        .add_systems(Startup, setup)
        .add_systems(Update, update_screen_size)
        // Title screen
        .add_systems(
            Update,
            (
                title_input.in_set(GameSet::Input),
                title_anim,
                title_auto_cycle,
                screen_fade_update,
            )
                .run_if(in_state(GameState::TitleScreen)),
        )
        .add_systems(
            OnEnter(GameState::TitleScreen),
            (
                |mut fade: ResMut<MusicFade>| request_subsong(&mut fade, 1),
                show_title_ui,
                |mut timer: ResMut<ScreenCycleTimer>| timer.0.reset(),
                start_screen_fade_in,
            ),
        )
        .add_systems(OnExit(GameState::TitleScreen), hide_title_ui)
        // Playing state
        .add_systems(
            OnEnter(GameState::Playing),
            (enter_playing, reset_attract_mode),
        )
        .add_systems(
            Update,
            (
                (player_movement, player_shooting).in_set(GameSet::Input),
                (
                    bullet_movement,
                    enemy_formation_movement,
                    diving_movement,
                    fading_text_update,
                    powerup_movement,
                )
                    .in_set(GameSet::Movement),
                (collisions, powerup_collection).in_set(GameSet::Collision),
                (enemy_shooting, initiate_dives, check_wave_complete).in_set(GameSet::Spawn),
                screen_shake_system,
                score_popup_system,
                combo_tick,
                screen_flash_system,
                invincibility_system,
            )
                .run_if(in_state(GameState::Playing)),
        )
        .add_systems(
            Update,
            (
                handle_sfx_events,
                handle_wave_complete,
                handle_player_hit,
                handle_enemy_killed,
                handle_extra_life,
                handle_powerup_collected,
            )
                .in_set(GameSet::Cleanup)
                .run_if(in_state(GameState::Playing)),
        )
        // Name entry state (high score)
        .add_systems(
            OnEnter(GameState::NameEntry),
            (enter_name_entry, activate_attract_mode),
        )
        .add_systems(
            Update,
            (name_entry_input, name_entry_blink, gameover_enemy_movement)
                .run_if(in_state(GameState::NameEntry)),
        )
        .add_systems(OnExit(GameState::NameEntry), exit_name_entry)
        // High scores state
        .add_systems(
            OnEnter(GameState::HighScores),
            (enter_high_scores, start_screen_fade_in),
        )
        .add_systems(
            Update,
            (
                high_scores_input,
                high_scores_fade,
                high_scores_auto_return,
                screen_fade_update,
                gameover_enemy_movement,
            )
                .run_if(in_state(GameState::HighScores)),
        )
        .add_systems(OnExit(GameState::HighScores), exit_high_scores)
        // Power-ups screen state
        .add_systems(
            OnEnter(GameState::PowerUpsScreen),
            (enter_powerups_screen, start_screen_fade_in),
        )
        .add_systems(
            Update,
            (
                powerups_screen_input,
                powerups_screen_fade,
                powerups_screen_auto_cycle,
                screen_fade_update,
                wavy_text_animation,
                wavy_sprite_animation,
            )
                .run_if(in_state(GameState::PowerUpsScreen)),
        )
        .add_systems(OnExit(GameState::PowerUpsScreen), exit_powerups_screen)
        // Enemy scores screen state
        .add_systems(
            OnEnter(GameState::EnemyScoresScreen),
            (enter_enemy_scores_screen, start_screen_fade_in),
        )
        .add_systems(
            Update,
            (
                enemy_scores_screen_input,
                enemy_scores_screen_fade,
                enemy_scores_screen_auto_cycle,
                screen_fade_update,
                wavy_text_animation,
                wavy_sprite_animation,
                animate_sprites,
            )
                .run_if(in_state(GameState::EnemyScoresScreen)),
        )
        .add_systems(
            OnExit(GameState::EnemyScoresScreen),
            exit_enemy_scores_screen,
        )
        // Game over state
        .add_systems(
            OnEnter(GameState::GameOver),
            (enter_gameover, activate_attract_mode),
        )
        .add_systems(
            Update,
            (
                gameover_enemy_movement,
                gameover_ui_animation,
                gameover_auto_return,
            )
                .run_if(in_state(GameState::GameOver)),
        )
        .add_systems(OnExit(GameState::GameOver), exit_gameover)
        // Global systems
        .add_systems(
            Update,
            (
                parallax_starfield,
                nebula_movement,
                game_restart.in_set(GameSet::Input),
                game_quit,
                music_toggle,
                crt_toggle,
                (
                    update_ui,
                    update_life_icons,
                    update_score_digits,
                    update_wave_digits,
                )
                    .run_if(resource_changed::<GameData>),
                animate_sprites,
                explosion_update,
            ),
        )
        .add_systems(
            Update,
            (update_crt_material, sync_render_target, music_crossfade),
        )
        .run();
}

fn setup(
    mut cmd: Commands,
    mut gist_assets: ResMut<Assets<GistAudio>>,
    mut setup_assets: SetupAssets,
    server: Res<AssetServer>,
    window: Single<&Window, With<PrimaryWindow>>,
    music_enabled: Res<MusicEnabled>,
) {
    let (ww, wh) = (window.width(), window.height());
    let (hw, hh) = (ww / 2.0, wh / 2.0);

    cmd.insert_resource(ScreenSize::from_window(&window));

    // Render target for CRT effect
    let extent = Extent3d {
        width: ww.max(1.0) as u32,
        height: wh.max(1.0) as u32,
        depth_or_array_layers: 1,
    };
    let mut rt_img = Image {
        texture_descriptor: TextureDescriptor {
            label: Some("game_scene"),
            size: extent,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba16Float,
            mip_level_count: 1,
            sample_count: 1,
            usage: TextureUsages::TEXTURE_BINDING
                | TextureUsages::RENDER_ATTACHMENT
                | TextureUsages::COPY_SRC,
            view_formats: &[],
        },
        ..default()
    };
    rt_img.resize(extent);
    let render_target = setup_assets.images.add(rt_img);
    cmd.insert_resource(SceneRenderTarget(render_target.clone()));

    // Cameras
    cmd.spawn((
        Camera2d,
        Camera {
            target: RenderTarget::Image(render_target.clone().into()),
            order: -1,
            clear_color: ClearColorConfig::Custom(Color::BLACK),
            ..default()
        },
        Transform::default(),
        GameCamera, // Marker for screen shake
        Name::new("GameCamera"),
    ));
    cmd.spawn((
        Camera2d,
        Camera {
            order: 0,
            clear_color: ClearColorConfig::Custom(Color::BLACK),
            ..default()
        },
        IsDefaultUiCamera,
        Name::new("DisplayCamera"),
    ));

    // CRT fullscreen quad
    let crt_mat = setup_assets.crt_materials.add(CrtMaterial {
        scene_texture: render_target,
        params: CrtParams {
            width: ww,
            height: wh,
            crt_enabled: 1,
            ..default()
        },
    });
    cmd.insert_resource(CrtMaterialHandle(crt_mat.clone()));
    cmd.spawn((
        Mesh2d(
            setup_assets
                .meshes
                .add(Mesh::from(Rectangle::new(2.0, 2.0))),
        ),
        MeshMaterial2d(crt_mat),
        Transform::from_scale(Vec3::new(hw, hh, 1.0)),
        CrtQuad,
    ));

    // Music
    let mut playback = Ym2149Playback::from_asset(server.load("sndh/Lethal_Xcess_(STe).sndh"));
    playback.set_volume(1.0);
    playback.set_subsong(1);
    if music_enabled.0 {
        playback.play();
    }
    cmd.spawn(playback);

    // SFX
    let gist = Arc::new(Mutex::new(GistPlayer::new()));
    cmd.insert_resource(GistRes(Arc::clone(&gist)));
    cmd.spawn(AudioPlayer(gist_assets.add(GistAudio {
        player: Arc::clone(&gist),
        volume: 0.25,
    })));
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/sfx/gist");
    cmd.insert_resource(Sfx {
        laser: GistSound::load(format!("{dir}/laser.snd")).unwrap(),
        explode: GistSound::load(format!("{dir}/explode.snd")).unwrap(),
        death: GistSound::load(format!("{dir}/falling.snd")).unwrap(),
    });

    // Load sprite assets
    let sprite_dir = "Mini Pixel Pack 3";
    cmd.insert_resource(SpriteAssets {
        player_texture: server.load(format!(
            "{sprite_dir}/Player ship/Player_ship (16 x 16).png"
        )),
        player_layout: setup_assets
            .atlas_layouts
            .add(TextureAtlasLayout::from_grid(
                UVec2::splat(16),
                3,
                1,
                None,
                None,
            )),
        booster_texture: server.load(format!("{sprite_dir}/Player ship/Boosters (16 x 16).png")),
        booster_layout: setup_assets
            .atlas_layouts
            .add(TextureAtlasLayout::from_grid(
                UVec2::splat(16),
                2,
                1,
                None,
                None,
            )),
        booster_left_texture: server.load(format!(
            "{sprite_dir}/Player ship/Boosters_left (16 x 16).png"
        )),
        booster_left_layout: setup_assets
            .atlas_layouts
            .add(TextureAtlasLayout::from_grid(
                UVec2::splat(16),
                2,
                1,
                None,
                None,
            )),
        booster_right_texture: server.load(format!(
            "{sprite_dir}/Player ship/Boosters_right (16 x 16).png"
        )),
        booster_right_layout: setup_assets
            .atlas_layouts
            .add(TextureAtlasLayout::from_grid(
                UVec2::splat(16),
                2,
                1,
                None,
                None,
            )),
        enemy_alan_texture: server.load(format!("{sprite_dir}/Enemies/Alan (16 x 16).png")),
        enemy_alan_layout: {
            let mut layout = TextureAtlasLayout::new_empty(UVec2::new(96, 16));
            for i in 0..6 {
                layout.add_texture(URect::new(i * 16, 0, i * 16 + 15, 16));
            }
            setup_assets.atlas_layouts.add(layout)
        },
        enemy_bonbon_texture: server.load(format!("{sprite_dir}/Enemies/Bon_Bon (16 x 16).png")),
        enemy_bonbon_layout: {
            let mut layout = TextureAtlasLayout::new_empty(UVec2::new(64, 16));
            for i in 0..4 {
                layout.add_texture(URect::new(i * 16, 0, i * 16 + 15, 16));
            }
            setup_assets.atlas_layouts.add(layout)
        },
        enemy_lips_texture: server.load(format!("{sprite_dir}/Enemies/Lips (16 x 16).png")),
        enemy_lips_layout: setup_assets
            .atlas_layouts
            .add(TextureAtlasLayout::from_grid(
                UVec2::splat(16),
                5,
                1,
                None,
                None,
            )),
        player_bullet_texture: server.load(format!(
            "{sprite_dir}/Projectiles/Player_beam (16 x 16).png"
        )),
        player_bullet_layout: setup_assets
            .atlas_layouts
            .add(TextureAtlasLayout::from_grid(
                UVec2::splat(16),
                2,
                1,
                None,
                None,
            )),
        triple_shot_texture: server.load(format!(
            "{sprite_dir}/Projectiles/Player_donut_shot (16 x 16).png"
        )),
        triple_shot_layout: setup_assets
            .atlas_layouts
            .add(TextureAtlasLayout::from_grid(
                UVec2::splat(16),
                2,
                1,
                None,
                None,
            )),
        power_shot_texture: server.load(format!(
            "{sprite_dir}/Projectiles/Player_square_shot (16 x 16).png"
        )),
        power_shot_layout: setup_assets
            .atlas_layouts
            .add(TextureAtlasLayout::from_grid(
                UVec2::splat(16),
                4,
                1,
                None,
                None,
            )),
        enemy_bullet_texture: server.load(format!(
            "{sprite_dir}/Projectiles/Enemy_projectile (16 x 16).png"
        )),
        enemy_bullet_layout: setup_assets
            .atlas_layouts
            .add(TextureAtlasLayout::from_grid(
                UVec2::splat(16),
                3,
                1,
                None,
                None,
            )),
        explosion_texture: server.load(format!("{sprite_dir}/Effects/Explosion (16 x 16).png")),
        explosion_layout: setup_assets
            .atlas_layouts
            .add(TextureAtlasLayout::from_grid(
                UVec2::splat(16),
                5,
                1,
                None,
                None,
            )),
        life_icon_texture: server.load(format!(
            "{sprite_dir}/UI objects/Player_life_icon (16 x 16).png"
        )),
        number_font_texture: server
            .load(format!("{sprite_dir}/UI objects/Number_font (8 x 8).png")),
        number_font_layout: setup_assets
            .atlas_layouts
            .add(TextureAtlasLayout::from_grid(
                UVec2::splat(8),
                5,
                2,
                None,
                None,
            )),
        powerup_texture: server.load(format!(
            "{sprite_dir}/Items/Circle_+_Square_+_Missile_pick-ups (16 x 16).png"
        )),
        powerup_layout: setup_assets
            .atlas_layouts
            .add(TextureAtlasLayout::from_grid(
                UVec2::splat(16),
                3,
                1,
                None,
                None,
            )),
    });

    // Load arcade font
    let fonts = FontAssets {
        arcade: server.load("fonts/ARCADECLASSIC.TTF"),
    };
    cmd.insert_resource(fonts.clone());

    // Create a 1x1 white pixel for stars using proper Image construction
    let star_img = Image::new_fill(
        bevy::render::render_resource::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        &[255, 255, 255, 255],
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    let star_texture = setup_assets.images.add(star_img);

    // Parallax Starfield - 3 layers with different depths
    let mut rng = SmallRng::from_os_rng();
    let star_configs: [(u8, i32, f32, f32, f32, f32); 3] = [
        // (layer, count, min_brightness, max_brightness, min_size, max_size)
        (0, 60, 0.3, 0.5, 1.0, 2.0), // Far layer - dimmer, small
        (1, 45, 0.5, 0.8, 1.5, 3.0), // Mid layer
        (2, 30, 0.8, 1.0, 2.5, 4.0), // Near layer - bright, large
    ];
    for (layer, count, min_b, max_b, min_s, max_s) in star_configs {
        for _ in 0..count {
            let b = rng.random_range(min_b..max_b);
            cmd.spawn((
                Sprite {
                    image: star_texture.clone(),
                    color: Color::srgb(b, b, b),
                    custom_size: Some(Vec2::splat(rng.random_range(min_s..max_s))),
                    ..default()
                },
                Transform::from_xyz(
                    rng.random_range(-hw..hw),
                    rng.random_range(-hh..hh),
                    0.1 + layer as f32 * 0.1, // Behind game entities (z=1.0)
                ),
                Star {
                    speed: rng.random_range(40.0..150.0),
                },
                StarLayer(layer),
            ));
        }
    }

    // Nebula background textures
    let nebula1: Handle<Image> = server.load("Mini Pixel Pack 3/Background_Nebula-0001.png");
    let nebula2: Handle<Image> = server.load("Mini Pixel Pack 3/Background_Nebula-0002.png");

    // Spawn nebula clouds at various positions (slower than stars for parallax)
    let nebula_configs: [(Handle<Image>, f32, f32, f32, f32, Color); 4] = [
        // (texture, x, y, scale, speed, color)
        (
            nebula1.clone(),
            -hw * 0.4,
            hh * 0.3,
            4.0,
            8.0,
            Color::srgba(1.0, 0.8, 1.0, 0.4),
        ),
        (
            nebula2.clone(),
            hw * 0.5,
            -hh * 0.2,
            3.5,
            10.0,
            Color::srgba(0.8, 1.0, 1.0, 0.35),
        ),
        (
            nebula1.clone(),
            hw * 0.2,
            hh * 0.5,
            3.0,
            6.0,
            Color::srgba(1.0, 1.0, 0.8, 0.3),
        ),
        (
            nebula2.clone(),
            -hw * 0.3,
            -hh * 0.4,
            3.5,
            12.0,
            Color::srgba(0.9, 0.8, 1.0, 0.35),
        ),
    ];
    for (texture, x, y, scale, speed, color) in nebula_configs {
        cmd.spawn((
            Sprite {
                image: texture,
                color,
                ..default()
            },
            Transform::from_xyz(x, y, 0.05).with_scale(Vec3::splat(scale)),
            Nebula { speed },
        ));
    }

    spawn_ui(&mut cmd, &fonts);
}
