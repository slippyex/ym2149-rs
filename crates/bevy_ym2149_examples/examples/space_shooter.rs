//! Space Shooter - A Galaxian-style retro game with CRT effect
//! Controls: Arrows (move), Space (fire), Enter (start), R (restart), M (music), C (CRT toggle), Q (quit)
//! CLI: -m false (disable music), --reset-hi-scores (reset high scores to defaults)
//!      --wave N (start at wave N), --boss (start directly at boss fight)

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
use bevy::ecs::schedule::IntoScheduleConfigs;
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

struct CliArgs {
    music_enabled: bool,
    reset_hi_scores: bool,
    start_wave: Option<u32>,
}

fn parse_args() -> CliArgs {
    let args: Vec<String> = env::args().collect();
    let mut music_enabled = true;
    let mut reset_hi_scores = false;
    let mut start_wave = None;

    let mut i = 1;
    while i < args.len() {
        if args[i] == "-m" && i + 1 < args.len() {
            music_enabled = args[i + 1].to_lowercase() != "false";
            i += 2;
        } else if args[i] == "--reset-hi-scores" {
            reset_hi_scores = true;
            i += 1;
        } else if args[i] == "--wave" && i + 1 < args.len() {
            start_wave = args[i + 1].parse().ok();
            i += 2;
        } else if args[i] == "--boss" {
            // Start at first boss wave (wave 5)
            start_wave = Some(BOSS_WAVE_INTERVAL);
            i += 1;
        } else {
            i += 1;
        }
    }
    CliArgs {
        music_enabled,
        reset_hi_scores,
        start_wave,
    }
}

fn main() {
    let cli = parse_args();

    // Reset high scores if requested
    if cli.reset_hi_scores {
        let default_scores = HighScoreList::default();
        default_scores.save();
        println!("High scores reset to defaults.");
    }

    App::new()
        .insert_resource(MusicEnabled(cli.music_enabled))
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
        .insert_resource(DebugStartWave(cli.start_wave))
        .insert_resource(GameTimers::default())
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
        .insert_resource(PlayerRespawnTimer::default())
        .insert_resource(PowerUpDropBoost::default())
        .insert_resource(FiringExhaustion::default())
        .insert_resource(PlayerEnergy::default())
        .insert_resource(WavePatternRotation::default())
        .insert_resource(PowerUpDropQueue::default())
        .insert_resource(PowerUpSpawnCooldown::default())
        .insert_resource(WaveIntermission::default())
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
        .add_systems(Update, (update_screen_size, screen_flash_system))
        // Title screen
        .add_systems(
            Update,
            title_input
                .in_set(GameSet::Input)
                .run_if(in_state(GameState::TitleScreen)),
        )
        .add_systems(
            Update,
            (
                ensure_title_screen_scene.run_if(in_state(GameState::TitleScreen)),
                title_anim.run_if(in_state(GameState::TitleScreen)),
                title_subtitle_anim.run_if(in_state(GameState::TitleScreen)),
                title_decor_update.run_if(in_state(GameState::TitleScreen)),
                title_flyby_update.run_if(in_state(GameState::TitleScreen)),
                title_auto_cycle.run_if(in_state(GameState::TitleScreen)),
                screen_fade_update.run_if(in_state(GameState::TitleScreen)),
                // Demoscene effects
                raster_bar_update.run_if(in_state(GameState::TitleScreen)),
            ),
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
        .add_systems(
            OnExit(GameState::TitleScreen),
            (hide_title_ui, exit_title_screen),
        )
        // Playing state
        .add_systems(
            OnEnter(GameState::Playing),
            (enter_playing, reset_attract_mode),
        )
        .add_systems(
            Update,
            (
                (player_movement, player_shooting)
                    .in_set(GameSet::Input)
                    .run_if(in_state(GameState::Playing)),
                (
                    bullet_movement,
                    enemy_entrance_system,
                    enemy_formation_movement,
                    boss_movement,
                    boss_escort_movement,
                    diving_movement,
                    spiral_movement,
                    fading_text_update,
                    powerup_movement,
                    powerup_animate,
                )
                    .in_set(GameSet::Movement)
                    .run_if(in_state(GameState::Playing)),
                collisions
                    .in_set(GameSet::Collision)
                    .run_if(in_state(GameState::Playing)),
                (powerup_collection, powerup_drop_boost_tick)
                    .in_set(GameSet::Collision)
                    .run_if(in_state(GameState::Playing)),
                (
                    enemy_shooting,
                    diver_shooting,
                    boss_shooting,
                    boss_escort_shooting,
                    boss_spawn_escorts,
                    boss_rage_update,
                    boss_charge_attack,
                    boss_phase_update,
                    boss_shield_update,
                    boss_shield_bubble_follow,
                    boss_homing_missile_spawn,
                    homing_missile_update,
                    boss_bomb_spawn,
                    boss_bomb_update,
                    bomb_explosion_update,
                    initiate_dives,
                    initiate_spirals,
                    spawn_queued_powerups,
                    wave_intermission_update,
                    check_wave_complete,
                )
                    .in_set(GameSet::Spawn)
                    .run_if(in_state(GameState::Playing)),
                screen_shake_system.run_if(in_state(GameState::Playing)),
                boss_death_fx_update.run_if(in_state(GameState::Playing)),
                score_popup_system.run_if(in_state(GameState::Playing)),
                combo_tick.run_if(in_state(GameState::Playing)),
                energy_bar_update.run_if(in_state(GameState::Playing)),
                boss_bar_update.run_if(in_state(GameState::Playing)),
                invincibility_system.run_if(in_state(GameState::Playing)),
                player_respawn_system.run_if(in_state(GameState::Playing)),
                shield_bubble_system.run_if(in_state(GameState::Playing)),
            ),
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
            (
                name_entry_input.run_if(in_state(GameState::NameEntry)),
                name_entry_blink.run_if(in_state(GameState::NameEntry)),
                gameover_enemy_movement.run_if(in_state(GameState::NameEntry)),
            ),
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
                high_scores_input.run_if(in_state(GameState::HighScores)),
                high_scores_fade.run_if(in_state(GameState::HighScores)),
                high_scores_auto_return.run_if(in_state(GameState::HighScores)),
                screen_fade_update.run_if(in_state(GameState::HighScores)),
                gameover_enemy_movement.run_if(in_state(GameState::HighScores)),
            ),
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
                powerups_screen_input.run_if(in_state(GameState::PowerUpsScreen)),
                powerups_screen_fade.run_if(in_state(GameState::PowerUpsScreen)),
                powerups_screen_auto_cycle.run_if(in_state(GameState::PowerUpsScreen)),
                screen_fade_update.run_if(in_state(GameState::PowerUpsScreen)),
                wavy_text_animation.run_if(in_state(GameState::PowerUpsScreen)),
                wavy_sprite_animation.run_if(in_state(GameState::PowerUpsScreen)),
            ),
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
                enemy_scores_screen_input.run_if(in_state(GameState::EnemyScoresScreen)),
                enemy_scores_screen_fade.run_if(in_state(GameState::EnemyScoresScreen)),
                enemy_scores_screen_auto_cycle.run_if(in_state(GameState::EnemyScoresScreen)),
                screen_fade_update.run_if(in_state(GameState::EnemyScoresScreen)),
                wavy_text_animation.run_if(in_state(GameState::EnemyScoresScreen)),
                wavy_sprite_animation.run_if(in_state(GameState::EnemyScoresScreen)),
                animate_sprites.run_if(in_state(GameState::EnemyScoresScreen)),
            ),
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
                gameover_enemy_movement.run_if(in_state(GameState::GameOver)),
                gameover_ui_animation.run_if(in_state(GameState::GameOver)),
                gameover_auto_return.run_if(in_state(GameState::GameOver)),
            ),
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
                combo_ui_update.run_if(in_state(GameState::Playing)),
                update_exhaustion_bar.run_if(in_state(GameState::Playing)),
                wave_transition_update.run_if(in_state(GameState::Playing)),
                wave_flyout_system.run_if(in_state(GameState::Playing)),
                animate_sprites,
                explosion_update,
                explosion_ring_update,
                trail_ghost_update,
                hit_flash_update,
                pickup_particle_update,
                booster_intensity_system.run_if(in_state(GameState::Playing)),
            ),
        )
        .add_systems(
            Update,
            (update_crt_material, sync_render_target, music_crossfade),
        )
        .run();
}

#[allow(clippy::too_many_arguments)]
fn setup(
    mut cmd: Commands,
    mut gist_assets: ResMut<Assets<GistAudio>>,
    mut setup_assets: SetupAssets,
    server: Res<AssetServer>,
    window: Single<&Window, With<PrimaryWindow>>,
    music_enabled: Res<MusicEnabled>,
    scores: Res<HighScoreList>,
    mut gd: ResMut<GameData>,
) {
    let (ww, wh) = (window.width(), window.height());
    let (hw, hh) = (ww / 2.0, wh / 2.0);

    cmd.insert_resource(ScreenSize::from_window(&window));
    gd.high_score = scores.entries.first().map(|e| e.score).unwrap_or(0);

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
            // `Rgba16Float` is overkill for this retro CRT pass and can cost a lot of bandwidth,
            // especially on high-DPI displays. `Rgba8UnormSrgb` is plenty and improves framerate.
            format: TextureFormat::Rgba8UnormSrgb,
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

    // Persistent screen-flash overlay (avoid per-event spawn/despawn).
    cmd.spawn((
        Sprite {
            color: Color::NONE,
            custom_size: Some(Vec2::new(ww * 2.0, wh * 2.0)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 50.0),
        Visibility::Hidden,
        space_shooter::components::ScreenFlashOverlay,
        Name::new("ScreenFlashOverlay"),
    ));

    // CRT fullscreen quad
    let crt_mat = setup_assets.crt_materials.add(CrtMaterial {
        scene_texture: render_target,
        params: CrtParams {
            width: ww,
            height: wh,
            crt_enabled: 1.0,
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
        powerup_pickup: GistSound::load(format!("{dir}/percbell.snd")).unwrap(),
    });

    // Create soft circle texture for shield bubble (64x64 with radial gradient)
    let bubble_size = 64u32;
    let mut bubble_data = Vec::with_capacity((bubble_size * bubble_size * 4) as usize);
    let center = bubble_size as f32 / 2.0;
    for y in 0..bubble_size {
        for x in 0..bubble_size {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let dist = (dx * dx + dy * dy).sqrt() / center;
            // Soft edge with glow - more transparent toward edge
            let alpha = if dist > 1.0 {
                0
            } else {
                ((1.0 - dist).powf(0.5) * 200.0) as u8
            };
            bubble_data.extend_from_slice(&[255, 255, 255, alpha]); // White with alpha
        }
    }
    let bubble_img = Image::new(
        bevy::render::render_resource::Extent3d {
            width: bubble_size,
            height: bubble_size,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        bubble_data,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    let bubble_texture = setup_assets.images.add(bubble_img);

    // Ring texture for explosions / impact bursts (64x64; alpha peaks at a radius).
    let ring_size = 64u32;
    let mut ring_data = Vec::with_capacity((ring_size * ring_size * 4) as usize);
    let center = ring_size as f32 / 2.0;
    for y in 0..ring_size {
        for x in 0..ring_size {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let dist = (dx * dx + dy * dy).sqrt() / center; // 0..~1

            let ring_center = 0.72;
            let ring_width = 0.08;
            let falloff = ((dist - ring_center) / ring_width).abs();
            let alpha = if dist > 1.0 {
                0
            } else {
                // Smooth peak around ring_center.
                (255.0 * (-falloff * falloff * 6.0).exp()).min(255.0) as u8
            };
            ring_data.extend_from_slice(&[255, 255, 255, alpha]);
        }
    }
    let ring_img = Image::new(
        bevy::render::render_resource::Extent3d {
            width: ring_size,
            height: ring_size,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        ring_data,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    let ring_texture = setup_assets.images.add(ring_img);

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
        powerup_texture: server.load(format!("{sprite_dir}/Items/Bonuses-0001.png")),
        powerup_layout: setup_assets
            .atlas_layouts
            .add(TextureAtlasLayout::from_grid(
                UVec2::splat(32),
                5,
                5,
                None,
                None,
            )),
        boss_texture: server.load(format!("{sprite_dir}/SpaceShip_Boss-0001.png")),
        boss_layout: setup_assets
            .atlas_layouts
            .add(TextureAtlasLayout::from_grid(
                UVec2::new(128, 105), // Smaller height to crop tightly
                2,
                3,
                Some(UVec2::new(0, 23)), // Vertical padding between rows
                Some(UVec2::new(0, 45)), // Skip flames + top padding
            )),
        bubble_texture,
        ring_texture,
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
            let (tint_r, tint_g, tint_b) = match layer {
                0 => (0.75, 0.85, 1.0), // Far: cool
                1 => (0.9, 0.95, 1.0),  // Mid: neutral
                _ => (1.0, 0.95, 0.8),  // Near: warm
            };
            let x = rng.random_range(-hw..hw);
            cmd.spawn((
                Sprite {
                    image: star_texture.clone(),
                    color: Color::srgb(b * tint_r, b * tint_g, b * tint_b),
                    custom_size: Some(Vec2::splat(rng.random_range(min_s..max_s))),
                    ..default()
                },
                Transform::from_xyz(
                    x,
                    rng.random_range(-hh..hh),
                    0.1 + layer as f32 * 0.1, // Behind game entities (z=1.0)
                ),
                Star {
                    speed: rng.random_range(40.0..150.0),
                },
                StarLayer(layer),
                ParallaxAnchor { base_x: x },
                StarTwinkle {
                    phase: rng.random_range(0.0..std::f32::consts::TAU),
                    speed: rng.random_range(0.8..2.4),
                    base: b,
                    amplitude: match layer {
                        0 => 0.10,
                        1 => 0.16,
                        _ => 0.22,
                    },
                    tint: Vec3::new(tint_r, tint_g, tint_b),
                },
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
            ParallaxAnchor { base_x: x },
            NebulaPulse {
                phase: rng.random_range(0.0..std::f32::consts::TAU),
                speed: rng.random_range(0.08..0.22),
                base_alpha: color.to_srgba().alpha,
                amplitude: 0.12,
            },
        ));
    }

    spawn_ui(&mut cmd, &fonts);
}
