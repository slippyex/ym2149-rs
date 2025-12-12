//! Space Shooter - A Galaxian-style retro game with CRT effect
//! Controls: Arrows (move), Space (fire), Enter (start), R (restart), M (music), C (CRT toggle), Esc (quit)

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
use std::sync::{Arc, Mutex};
use ym2149_gist_replayer::{GistPlayer, GistSound};

use space_shooter::{
    audio::GistAudio,
    components::*,
    config::*,
    crt::{crt_toggle, sync_render_target, update_crt_material, CrtMaterial, CrtParams},
    resources::*,
    systems::*,
    ui::*,
};

fn main() {
    App::new()
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
        .add_message::<PlaySfxMsg>()
        .add_message::<WaveCompleteMsg>()
        .add_message::<PlayerHitMsg>()
        .add_message::<EnemyKilledMsg>()
        .add_message::<ExtraLifeMsg>()
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
            (title_input.in_set(GameSet::Input), title_anim)
                .run_if(in_state(GameState::TitleScreen)),
        )
        .add_systems(
            OnEnter(GameState::TitleScreen),
            |mut fade: ResMut<MusicFade>| request_subsong(&mut fade, 1),
        )
        .add_systems(OnExit(GameState::TitleScreen), hide_title_ui)
        // Playing state
        .add_systems(OnEnter(GameState::Playing), enter_playing)
        .add_systems(
            Update,
            (
                (player_movement, player_shooting).in_set(GameSet::Input),
                (
                    bullet_movement,
                    enemy_formation_movement,
                    diving_movement,
                    fading_text_update,
                )
                    .in_set(GameSet::Movement),
                collisions.in_set(GameSet::Collision),
                (enemy_shooting, initiate_dives, check_wave_complete).in_set(GameSet::Spawn),
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
            )
                .in_set(GameSet::Cleanup)
                .run_if(in_state(GameState::Playing)),
        )
        // Game over state
        .add_systems(OnEnter(GameState::GameOver), enter_gameover)
        .add_systems(
            Update,
            (gameover_enemy_movement, gameover_ui_animation).run_if(in_state(GameState::GameOver)),
        )
        .add_systems(OnExit(GameState::GameOver), exit_gameover)
        // Global systems
        .add_systems(
            Update,
            (
                starfield,
                game_input.in_set(GameSet::Input),
                music_toggle,
                crt_toggle,
                (update_ui, update_life_icons, update_score_digits, update_wave_digits)
                    .run_if(resource_changed::<GameData>),
                animate_sprites,
                explosion_update,
            ),
        )
        .add_systems(Update, (update_crt_material, sync_render_target, music_crossfade))
        .run();
}

fn setup(
    mut cmd: Commands,
    mut assets: ResMut<Assets<GistAudio>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut crt_materials: ResMut<Assets<CrtMaterial>>,
    mut atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    server: Res<AssetServer>,
    window: Single<&Window, With<PrimaryWindow>>,
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
    let render_target = images.add(rt_img);
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
    let crt_mat = crt_materials.add(CrtMaterial {
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
        Mesh2d(meshes.add(Mesh::from(Rectangle::new(2.0, 2.0)))),
        MeshMaterial2d(crt_mat),
        Transform::from_scale(Vec3::new(hw, hh, 1.0)),
        CrtQuad,
    ));

    // Music
    let mut playback = Ym2149Playback::from_asset(server.load("sndh/Lethal_Xcess_(STe).sndh"));
    playback.set_volume(1.0);
    playback.set_subsong(1);
    playback.play();
    cmd.spawn(playback);

    // SFX
    let gist = Arc::new(Mutex::new(GistPlayer::new()));
    cmd.insert_resource(GistRes(Arc::clone(&gist)));
    cmd.spawn(AudioPlayer(assets.add(GistAudio {
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
        player_texture: server.load(format!("{sprite_dir}/Player ship/Player_ship (16 x 16).png")),
        player_layout: atlas_layouts.add(TextureAtlasLayout::from_grid(
            UVec2::splat(16), 3, 1, None, None,
        )),
        booster_texture: server.load(format!("{sprite_dir}/Player ship/Boosters (16 x 16).png")),
        booster_layout: atlas_layouts.add(TextureAtlasLayout::from_grid(
            UVec2::splat(16), 2, 1, None, None,
        )),
        enemy_alan_texture: server.load(format!("{sprite_dir}/Enemies/Alan (16 x 16).png")),
        enemy_alan_layout: {
            let mut layout = TextureAtlasLayout::new_empty(UVec2::new(96, 16));
            for i in 0..6 {
                layout.add_texture(URect::new(i * 16, 0, i * 16 + 15, 16));
            }
            atlas_layouts.add(layout)
        },
        enemy_bonbon_texture: server.load(format!("{sprite_dir}/Enemies/Bon_Bon (16 x 16).png")),
        enemy_bonbon_layout: {
            let mut layout = TextureAtlasLayout::new_empty(UVec2::new(64, 16));
            for i in 0..4 {
                layout.add_texture(URect::new(i * 16, 0, i * 16 + 15, 16));
            }
            atlas_layouts.add(layout)
        },
        enemy_lips_texture: server.load(format!("{sprite_dir}/Enemies/Lips (16 x 16).png")),
        enemy_lips_layout: atlas_layouts.add(TextureAtlasLayout::from_grid(
            UVec2::splat(16), 5, 1, None, None,
        )),
        player_bullet_texture: server.load(format!("{sprite_dir}/Projectiles/Player_beam (16 x 16).png")),
        player_bullet_layout: atlas_layouts.add(TextureAtlasLayout::from_grid(
            UVec2::splat(16), 2, 1, None, None,
        )),
        enemy_bullet_texture: server.load(format!("{sprite_dir}/Projectiles/Enemy_projectile (16 x 16).png")),
        enemy_bullet_layout: atlas_layouts.add(TextureAtlasLayout::from_grid(
            UVec2::splat(16), 3, 1, None, None,
        )),
        explosion_texture: server.load(format!("{sprite_dir}/Effects/Explosion (16 x 16).png")),
        explosion_layout: atlas_layouts.add(TextureAtlasLayout::from_grid(
            UVec2::splat(16), 5, 1, None, None,
        )),
        life_icon_texture: server.load(format!("{sprite_dir}/UI objects/Player_life_icon (16 x 16).png")),
        number_font_texture: server.load(format!("{sprite_dir}/UI objects/Number_font (8 x 8).png")),
        number_font_layout: atlas_layouts.add(TextureAtlasLayout::from_grid(
            UVec2::splat(8), 5, 2, None, None,
        )),
    });

    // Starfield
    let mut rng = SmallRng::from_os_rng();
    for _ in 0..100 {
        let b = rng.random_range(0.3..1.0);
        cmd.spawn((
            Sprite {
                color: Color::srgba(b, b, b, b),
                custom_size: Some(Vec2::splat(rng.random_range(1.0..3.0))),
                ..default()
            },
            Transform::from_xyz(rng.random_range(-hw..hw), rng.random_range(-hh..hh), 0.0),
            Star { speed: rng.random_range(20.0..100.0) },
            GameEntity,
        ));
    }

    spawn_ui(&mut cmd);
}
