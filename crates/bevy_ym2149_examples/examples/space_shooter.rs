//! Space Shooter - A Galaxian-style retro game with CRT effect
//! Controls: Arrows (move), Space (fire), Enter (start), R (restart), M (music), C (CRT toggle), Esc (quit)

use bevy::audio::{AddAudioSource, AudioPlayer, Decodable, Source};
use bevy::camera::{ClearColorConfig, RenderTarget};
use bevy::math::primitives::Rectangle;
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, Extent3d, ShaderType, TextureDescriptor, TextureDimension, TextureFormat,
    TextureUsages,
};
use bevy::shader::ShaderRef;
use bevy::sprite_render::{Material2d, Material2dPlugin, MeshMaterial2d};
use bevy::window::PrimaryWindow;
use bevy_mesh::Mesh2d;
use bevy_ym2149::{Ym2149Playback, Ym2149Plugin};
use bevy_ym2149_examples::{embedded_asset_plugin, example_plugins};
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use std::sync::{Arc, Mutex};
use ym2149_gist_replayer::{GistPlayer, GistSound};

// Constants
const PLAYER_SPEED: f32 = 400.0;
const PLAYER_SIZE: Vec2 = Vec2::new(48.0, 48.0); // 16x16 sprite scaled 3x
const BULLET_SPEED: f32 = 600.0;
const BULLET_SIZE: Vec2 = Vec2::new(24.0, 24.0); // 16x16 sprite scaled 1.5x
const ENEMY_SIZE: Vec2 = Vec2::new(48.0, 48.0); // 16x16 sprite scaled 3x
const ENEMY_BULLET_SPEED: f32 = 300.0;
const ENEMY_SPACING: Vec2 = Vec2::new(56.0, 52.0);
const STARTING_LIVES: u32 = 3;
const EXTRA_LIFE_SCORE: u32 = 3000;
const FADE_DURATION: f32 = 2.0;
const SPRITE_SCALE: f32 = 3.0;
const ANIM_FPS: f32 = 8.0;

// ============================================================================
// States & Messages
// ============================================================================

#[derive(States, Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
enum GameState {
    #[default]
    TitleScreen,
    Playing,
    GameOver,
}

#[derive(bevy::ecs::message::Message)]
struct PlaySfxMsg(SfxType);
#[derive(bevy::ecs::message::Message)]
struct WaveCompleteMsg;
#[derive(bevy::ecs::message::Message)]
struct PlayerHitMsg;
#[derive(bevy::ecs::message::Message)]
struct EnemyKilledMsg(u32);
#[derive(bevy::ecs::message::Message)]
struct ExtraLifeMsg;

#[derive(Clone, Copy)]
enum SfxType {
    Laser,
    Explode,
    Death,
}

// ============================================================================
// Resources
// ============================================================================

/// Cached window dimensions - updated on resize, avoids per-frame Window queries
#[derive(Resource)]
struct ScreenSize {
    width: f32,
    height: f32,
    half_width: f32,
    half_height: f32,
}

impl ScreenSize {
    fn from_window(w: &Window) -> Self {
        Self {
            width: w.width(),
            height: w.height(),
            half_width: w.width() / 2.0,
            half_height: w.height() / 2.0,
        }
    }
}

#[derive(Resource)]
struct GameData {
    score: u32,
    high_score: u32,
    lives: u32,
    wave: u32,
    shoot_timer: Timer,
    enemy_shoot_timer: Timer,
    enemy_direction: f32,
    dive_timer: Timer,
    next_extra_life: u32,
}

impl GameData {
    fn new() -> Self {
        Self {
            lives: STARTING_LIVES,
            wave: 1,
            enemy_direction: 1.0,
            next_extra_life: EXTRA_LIFE_SCORE,
            shoot_timer: Timer::from_seconds(0.0, TimerMode::Once),
            enemy_shoot_timer: Timer::from_seconds(1.5, TimerMode::Once),
            dive_timer: Timer::from_seconds(3.0, TimerMode::Once),
            score: 0,
            high_score: 0,
        }
    }
    fn reset(&mut self) {
        let hs = self.high_score;
        *self = Self::new();
        self.high_score = hs;
    }
    fn dive_interval(&self) -> f32 {
        (3.0 - (self.wave as f32 - 1.0) * 0.2).max(0.8)
    }
    fn max_divers(&self) -> usize {
        (1 + self.wave as usize / 2).min(5)
    }
    fn enemy_shoot_rate(&self) -> f32 {
        (1.5 - (self.wave as f32 - 1.0) * 0.1).max(0.5)
    }
}

#[derive(Resource, Default)]
struct MusicFade {
    target_subsong: Option<usize>,
    phase: FadePhase,
    timer: f32,
}

#[derive(Default, Clone, Copy, PartialEq)]
enum FadePhase {
    #[default]
    Idle,
    FadeOut,
    FadeIn,
}

#[derive(Resource)]
struct CrtState {
    enabled: bool,
}

#[derive(Resource)]
struct CrtMaterialHandle(Handle<CrtMaterial>);

#[derive(Resource)]
struct SceneRenderTarget(Handle<Image>);

#[derive(Resource)]
struct Sfx {
    laser: GistSound,
    explode: GistSound,
    death: GistSound,
}

#[derive(Resource, Clone)]
struct GistRes(Arc<Mutex<GistPlayer>>);

/// All sprite sheet assets for the game
#[derive(Resource)]
struct SpriteAssets {
    player_texture: Handle<Image>,
    player_layout: Handle<TextureAtlasLayout>,
    booster_texture: Handle<Image>,
    booster_layout: Handle<TextureAtlasLayout>,
    enemy_alan_texture: Handle<Image>,
    enemy_alan_layout: Handle<TextureAtlasLayout>,
    enemy_bonbon_texture: Handle<Image>,
    enemy_bonbon_layout: Handle<TextureAtlasLayout>,
    enemy_lips_texture: Handle<Image>,
    enemy_lips_layout: Handle<TextureAtlasLayout>,
    player_bullet_texture: Handle<Image>,
    player_bullet_layout: Handle<TextureAtlasLayout>,
    enemy_bullet_texture: Handle<Image>,
    enemy_bullet_layout: Handle<TextureAtlasLayout>,
    explosion_texture: Handle<Image>,
    explosion_layout: Handle<TextureAtlasLayout>,
    life_icon_texture: Handle<Image>,
    number_font_texture: Handle<Image>,
    number_font_layout: Handle<TextureAtlasLayout>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum EnemyType {
    Alan,
    BonBon,
    Lips,
}

// ============================================================================
// Components
// ============================================================================

#[derive(Component)]
struct Player;

/// Marker: bullet fired by player
#[derive(Component)]
struct PlayerBullet;

/// Marker: bullet fired by enemy
#[derive(Component)]
struct EnemyBullet;

#[derive(Component)]
struct Enemy {
    points: u32,
}

/// Animation frame indices (first..=last)
#[derive(Component)]
struct AnimationIndices {
    first: usize,
    last: usize,
}

/// Timer for sprite animation
#[derive(Component, Deref, DerefMut)]
struct AnimationTimer(Timer);

/// Booster flame attached to player
#[derive(Component)]
struct Booster;

/// Explosion effect
#[derive(Component)]
struct Explosion;

#[derive(Component)]
struct Star {
    speed: f32,
}

#[derive(Component)]
struct DivingEnemy {
    target_x: f32,
    returning: bool,
    start_y: f32,
    original_x: f32,
    progress: f32,
    amplitude: f32,
    curve_dir: f32,
}

#[derive(Component)]
struct GameOverEnemy {
    phase: f32,
    amplitude: f32,
    frequency: f32,
    base_pos: Vec2,
    delay: f32,
    started: bool,
}

#[derive(Component)]
struct GameOverUi {
    base_top: f32,
}

#[derive(Component)]
struct FadingText {
    timer: Timer,
    color: Color,
    spawn_enemies: bool,
}

#[derive(Component)]
struct GameEntity;

#[derive(Component)]
struct CrtQuad;

/// Individual life icon sprite
#[derive(Component)]
struct LifeIcon;

/// Individual digit sprite with position index
#[derive(Component)]
struct DigitSprite {
    position: usize,
}

/// Score type marker for digit sprites
#[derive(Component, Clone, Copy, PartialEq)]
enum ScoreType {
    Score,
    HighScore,
}

/// Wave digit sprite marker
#[derive(Component)]
struct WaveDigit;

#[derive(Component, Clone, Copy, PartialEq)]
enum UiMarker {
    Score,
    High,
    Lives,
    Wave,
    GameOver,
    Title,
    PressEnter,
    Subtitle,
}

// ============================================================================
// CRT Material
// ============================================================================

#[derive(AsBindGroup, TypePath, Debug, Clone, Asset)]
struct CrtMaterial {
    #[texture(0)]
    #[sampler(1)]
    scene_texture: Handle<Image>,
    #[uniform(2)]
    params: CrtParams,
}

impl Material2d for CrtMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path("shaders/crt_post.wgsl".into())
    }
}

#[derive(ShaderType, Clone, Copy, Debug, Default)]
struct CrtParams {
    time: f32,
    width: f32,
    height: f32,
    crt_enabled: u32,
}

// ============================================================================
// Audio
// ============================================================================

#[derive(Asset, TypePath, Clone)]
struct GistAudio {
    player: Arc<Mutex<GistPlayer>>,
    volume: f32,
}

struct GistDec {
    player: Arc<Mutex<GistPlayer>>,
    volume: f32,
}

impl Decodable for GistAudio {
    type DecoderItem = f32;
    type Decoder = GistDec;
    fn decoder(&self) -> Self::Decoder {
        GistDec {
            player: Arc::clone(&self.player),
            volume: self.volume,
        }
    }
}

impl Iterator for GistDec {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        Some(self.player.lock().unwrap().generate_samples(1)[0] * self.volume)
    }
}

impl Source for GistDec {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        1
    }
    fn sample_rate(&self) -> u32 {
        44100
    }
    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
    fn try_seek(&mut self, _: std::time::Duration) -> Result<(), bevy::audio::SeekError> {
        Ok(())
    }
}

// ============================================================================
// System Sets for Ordering
// ============================================================================

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
enum GameSet {
    Input,
    Movement,
    Collision,
    Spawn,
    Cleanup,
}

// ============================================================================
// Main
// ============================================================================

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
        // Configure system ordering
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
        // Global systems (all states)
        .add_systems(
            Update,
            (
                starfield,
                game_input.in_set(GameSet::Input),
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

// ============================================================================
// Setup
// ============================================================================

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

    // Cache screen size
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

    // Cameras: game renders to texture, display camera shows CRT effect
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
        // Player ship: 3 frames (16x16 each)
        player_texture: server.load(format!(
            "{sprite_dir}/Player ship/Player_ship (16 x 16).png"
        )),
        player_layout: atlas_layouts.add(TextureAtlasLayout::from_grid(
            UVec2::splat(16),
            3,
            1,
            None,
            None,
        )),
        // Boosters: 2 frames (16x16 each)
        booster_texture: server.load(format!("{sprite_dir}/Player ship/Boosters (16 x 16).png")),
        booster_layout: atlas_layouts.add(TextureAtlasLayout::from_grid(
            UVec2::splat(16),
            2,
            1,
            None,
            None,
        )),
        // Enemy Alan: 6 frames (custom layout - avoid 1px artifact on right)
        enemy_alan_texture: server.load(format!("{sprite_dir}/Enemies/Alan (16 x 16).png")),
        enemy_alan_layout: {
            let mut layout = TextureAtlasLayout::new_empty(UVec2::new(96, 16));
            for i in 0..6 {
                layout.add_texture(URect::new(i * 16, 0, i * 16 + 15, 16));
            }
            atlas_layouts.add(layout)
        },
        // Enemy Bon_Bon: 4 frames (custom layout - last frame has 1px artifact on right)
        enemy_bonbon_texture: server.load(format!("{sprite_dir}/Enemies/Bon_Bon (16 x 16).png")),
        enemy_bonbon_layout: {
            let mut layout = TextureAtlasLayout::new_empty(UVec2::new(64, 16));
            for i in 0..4 {
                // Use 15px width to avoid rightmost pixel artifact
                layout.add_texture(URect::new(i * 16, 0, i * 16 + 15, 16));
            }
            atlas_layouts.add(layout)
        },
        // Enemy Lips: 5 frames (80px / 16px)
        enemy_lips_texture: server.load(format!("{sprite_dir}/Enemies/Lips (16 x 16).png")),
        enemy_lips_layout: atlas_layouts.add(TextureAtlasLayout::from_grid(
            UVec2::splat(16),
            5,
            1,
            None,
            None,
        )),
        // Player bullet: 2 frames
        player_bullet_texture: server.load(format!(
            "{sprite_dir}/Projectiles/Player_beam (16 x 16).png"
        )),
        player_bullet_layout: atlas_layouts.add(TextureAtlasLayout::from_grid(
            UVec2::splat(16),
            2,
            1,
            None,
            None,
        )),
        // Enemy bullet: 3 variants (use first one)
        enemy_bullet_texture: server.load(format!(
            "{sprite_dir}/Projectiles/Enemy_projectile (16 x 16).png"
        )),
        enemy_bullet_layout: atlas_layouts.add(TextureAtlasLayout::from_grid(
            UVec2::splat(16),
            3,
            1,
            None,
            None,
        )),
        // Explosion: 5 frames
        explosion_texture: server.load(format!("{sprite_dir}/Effects/Explosion (16 x 16).png")),
        explosion_layout: atlas_layouts.add(TextureAtlasLayout::from_grid(
            UVec2::splat(16),
            5,
            1,
            None,
            None,
        )),
        // Life icon
        life_icon_texture: server.load(format!(
            "{sprite_dir}/UI objects/Player_life_icon (16 x 16).png"
        )),
        // Number font: 5 columns x 2 rows (digits 0-4 top, 5-9 bottom)
        number_font_texture: server
            .load(format!("{sprite_dir}/UI objects/Number_font (8 x 8).png")),
        number_font_layout: atlas_layouts.add(TextureAtlasLayout::from_grid(
            UVec2::splat(8),
            5,
            2,
            None,
            None,
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
            Star {
                speed: rng.random_range(20.0..100.0),
            },
            GameEntity,
        ));
    }

    // UI elements
    spawn_ui(&mut cmd);
}

fn spawn_ui(cmd: &mut Commands) {
    let ui_elements = [
        (
            "",
            24.0,
            Color::WHITE,
            Val::Px(15.0),
            Some(Val::Px(20.0)),
            None,
            false,
            UiMarker::Score,
        ), // Placeholder - rendered with sprites
        (
            "",
            24.0,
            Color::srgb(1.0, 0.8, 0.0),
            Val::Px(15.0),
            None,
            None,
            false,
            UiMarker::High,
        ), // Placeholder - rendered with sprites
        (
            "",
            0.0,
            Color::NONE,
            Val::Px(0.0),
            None,
            None,
            false,
            UiMarker::Lives,
        ), // Not used anymore
        (
            "WAVE 1",
            18.0,
            Color::srgb(0.6, 0.6, 1.0),
            Val::Px(45.0),
            Some(Val::Px(20.0)),
            None,
            false,
            UiMarker::Wave,
        ),
        (
            "GAME OVER\n\nPress R to Restart",
            48.0,
            Color::srgb(1.0, 0.2, 0.2),
            Val::Percent(45.0),
            None,
            None,
            false,
            UiMarker::GameOver,
        ),
        (
            "SPACE SHOOTER",
            64.0,
            Color::srgb(0.2, 0.8, 1.0),
            Val::Percent(25.0),
            None,
            None,
            true,
            UiMarker::Title,
        ),
        (
            "Press ENTER to Start",
            32.0,
            Color::srgb(1.0, 1.0, 0.2),
            Val::Percent(50.0),
            None,
            None,
            true,
            UiMarker::PressEnter,
        ),
        (
            "Music: Mad Max - Lethal Xcess (STe) | C: Toggle CRT",
            18.0,
            Color::srgba(0.7, 0.7, 0.7, 0.9),
            Val::Percent(65.0),
            None,
            None,
            true,
            UiMarker::Subtitle,
        ),
    ];

    for (txt, size, color, top, left, right, vis, marker) in ui_elements {
        let mut node = Node {
            position_type: PositionType::Absolute,
            top,
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        };
        if let Some(l) = left {
            node.left = l;
            node.width = Val::Auto;
        }
        if let Some(r) = right {
            node.right = r;
            node.width = Val::Auto;
        }
        cmd.spawn((
            Text::new(txt),
            TextFont {
                font_size: size,
                ..default()
            },
            TextColor(color),
            TextLayout::new_with_justify(bevy::text::Justify::Center),
            node,
            if vis {
                Visibility::Visible
            } else {
                Visibility::Hidden
            },
            marker,
        ));
    }
}

// ============================================================================
// Spawning Helpers
// ============================================================================

fn spawn_player(cmd: &mut Commands, hh: f32, sprites: &SpriteAssets) {
    // Spawn player ship (sprite frame controlled by movement: 0=left, 1=center, 2=right)
    let player_id = cmd
        .spawn((
            Sprite::from_atlas_image(
                sprites.player_texture.clone(),
                TextureAtlas {
                    layout: sprites.player_layout.clone(),
                    index: 1, // Start with center sprite
                },
            ),
            Transform::from_xyz(0.0, -hh + 60.0, 1.0).with_scale(Vec3::splat(SPRITE_SCALE)),
            Player,
            GameEntity,
        ))
        .id();

    // Spawn booster flame as child of player
    cmd.spawn((
        Sprite::from_atlas_image(
            sprites.booster_texture.clone(),
            TextureAtlas {
                layout: sprites.booster_layout.clone(),
                index: 0,
            },
        ),
        Transform::from_xyz(0.0, -12.0, -0.1), // Position below ship (in local coords)
        ChildOf(player_id),
        Booster,
        AnimationIndices { first: 0, last: 1 },
        AnimationTimer(Timer::from_seconds(0.08, TimerMode::Repeating)),
    ));
}

fn spawn_enemies(cmd: &mut Commands, sprites: &SpriteAssets) {
    let start_x = -(7.0 * ENEMY_SPACING.x) / 2.0;
    // Row config: (points, enemy_type)
    let rows: [(u32, EnemyType); 4] = [
        (40, EnemyType::Lips),   // Top row - pink lips (hardest)
        (30, EnemyType::BonBon), // Yellow candy
        (20, EnemyType::Alan),   // Green alien
        (10, EnemyType::Alan),   // Bottom row - green alien (easiest)
    ];
    for (row, (pts, enemy_type)) in rows.iter().enumerate() {
        let (texture, layout, last_frame) = match enemy_type {
            EnemyType::Alan => (
                sprites.enemy_alan_texture.clone(),
                sprites.enemy_alan_layout.clone(),
                5,
            ), // 6 frames
            EnemyType::BonBon => (
                sprites.enemy_bonbon_texture.clone(),
                sprites.enemy_bonbon_layout.clone(),
                3,
            ), // 4 frames
            EnemyType::Lips => (
                sprites.enemy_lips_texture.clone(),
                sprites.enemy_lips_layout.clone(),
                4,
            ), // 5 frames
        };
        for col in 0..8 {
            cmd.spawn((
                Sprite::from_atlas_image(
                    texture.clone(),
                    TextureAtlas {
                        layout: layout.clone(),
                        index: 0,
                    },
                ),
                Transform::from_xyz(
                    start_x + col as f32 * ENEMY_SPACING.x,
                    200.0 - row as f32 * ENEMY_SPACING.y,
                    1.0,
                )
                .with_scale(Vec3::splat(SPRITE_SCALE)),
                Enemy { points: *pts },
                GameEntity,
                AnimationIndices {
                    first: 0,
                    last: last_frame,
                },
                AnimationTimer(Timer::from_seconds(1.0 / ANIM_FPS, TimerMode::Repeating)),
            ));
        }
    }
}

fn spawn_fading_text(
    cmd: &mut Commands,
    text: &str,
    duration: f32,
    color: Color,
    spawn_enemies: bool,
) {
    if spawn_enemies {
        cmd.spawn((
            Sprite {
                color: Color::NONE,
                custom_size: Some(Vec2::ZERO),
                ..default()
            },
            Transform::default(),
            FadingText {
                timer: Timer::from_seconds(duration, TimerMode::Once),
                color,
                spawn_enemies: true,
            },
            GameEntity,
        ));
    }
    cmd.spawn((
        Text::new(text),
        TextFont {
            font_size: if spawn_enemies { 72.0 } else { 48.0 },
            ..default()
        },
        TextColor(color),
        TextLayout::new_with_justify(bevy::text::Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(if spawn_enemies { 45.0 } else { 40.0 }),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        FadingText {
            timer: Timer::from_seconds(duration, TimerMode::Once),
            color,
            spawn_enemies: false,
        },
    ));
}

const LIFE_ICON_SCALE: f32 = 2.0;
const LIFE_ICON_SPACING: f32 = 36.0; // 16 * 2 + 4px gap
const DIGIT_SCALE: f32 = 3.0;
const DIGIT_SPACING: f32 = 26.0; // 8 * 3 + 2px gap
const WAVE_DIGIT_SCALE: f32 = 2.5;

/// Spawn life icon sprites (world-space, top-right corner, right-aligned)
fn spawn_life_icons(cmd: &mut Commands, sprites: &SpriteAssets, screen: &ScreenSize, lives: u32) {
    // Position in top-right corner (world space coordinates)
    let base_x = screen.half_width - 30.0; // Right edge with margin
    let y = screen.half_height - 30.0; // Top edge with margin

    for i in 0..lives {
        // Right-aligned: newest lives appear to the left
        let x = base_x - (i as f32) * LIFE_ICON_SPACING;
        cmd.spawn((
            Sprite::from_image(sprites.life_icon_texture.clone()),
            Transform::from_xyz(x, y, 10.0).with_scale(Vec3::splat(LIFE_ICON_SCALE)),
            LifeIcon,
            GameEntity,
        ));
    }
}

/// Get the atlas index for a digit (0-9)
/// Number font layout: 1-5 in top row (indices 0-4), 6-9,0 in bottom row (indices 5-9)
fn digit_to_atlas_index(digit: u8) -> usize {
    if digit == 0 {
        9 // 0 is at the last position
    } else {
        (digit - 1) as usize // 1-9 map to indices 0-8
    }
}

/// Spawn score digit sprites (world-space)
fn spawn_score_digits(
    cmd: &mut Commands,
    sprites: &SpriteAssets,
    screen: &ScreenSize,
    score_type: ScoreType,
    value: u32,
) {
    // Position based on score type
    let (base_x, y) = match score_type {
        ScoreType::Score => (-screen.half_width + 30.0, screen.half_height - 25.0), // Top-left
        ScoreType::HighScore => (-40.0, screen.half_height - 25.0), // Top-center (offset left to center the digits)
    };

    // Convert score to digits (pad to 6 digits for consistent display)
    let digits: Vec<u8> = format!("{:06}", value.min(999999))
        .chars()
        .map(|c| c.to_digit(10).unwrap() as u8)
        .collect();

    for (i, &digit) in digits.iter().enumerate() {
        let x = base_x + (i as f32) * DIGIT_SPACING;
        cmd.spawn((
            Sprite::from_atlas_image(
                sprites.number_font_texture.clone(),
                TextureAtlas {
                    layout: sprites.number_font_layout.clone(),
                    index: digit_to_atlas_index(digit),
                },
            ),
            Transform::from_xyz(x, y, 10.0).with_scale(Vec3::splat(DIGIT_SCALE)),
            DigitSprite { position: i },
            score_type,
            GameEntity,
        ));
    }
}

/// Spawn wave number digits (world-space, below score on left)
fn spawn_wave_digits(cmd: &mut Commands, sprites: &SpriteAssets, screen: &ScreenSize, wave: u32) {
    let base_x = -screen.half_width + 30.0; // Same x as score
    let y = screen.half_height - 60.0; // Below the score
    let digit_spacing = 22.0; // 8 * 2.5 + 2px gap

    // Convert wave to 2-digit string (waves 1-99)
    let digits: Vec<u8> = format!("{:02}", wave.min(99))
        .chars()
        .map(|c| c.to_digit(10).unwrap() as u8)
        .collect();

    for (i, &digit) in digits.iter().enumerate() {
        let x = base_x + (i as f32) * digit_spacing;
        cmd.spawn((
            Sprite::from_atlas_image(
                sprites.number_font_texture.clone(),
                TextureAtlas {
                    layout: sprites.number_font_layout.clone(),
                    index: digit_to_atlas_index(digit),
                },
            ),
            Transform::from_xyz(x, y, 10.0).with_scale(Vec3::splat(WAVE_DIGIT_SCALE)),
            WaveDigit,
            GameEntity,
        ));
    }
}

// ============================================================================
// Screen Size Management
// ============================================================================

fn update_screen_size(
    window: Single<&Window, With<PrimaryWindow>>,
    mut screen: ResMut<ScreenSize>,
) {
    if screen.width != window.width() || screen.height != window.height() {
        *screen = ScreenSize::from_window(&window);
    }
}

// ============================================================================
// CRT Systems
// ============================================================================

fn sync_render_target(
    screen: Res<ScreenSize>,
    mut images: ResMut<Assets<Image>>,
    rt: Option<Res<SceneRenderTarget>>,
    mut crt_q: Query<&mut Transform, With<CrtQuad>>,
) {
    let Some(rt) = rt else { return };
    if let Some(img) = images.get_mut(&rt.0) {
        let (ww, wh) = (screen.width.max(1.0) as u32, screen.height.max(1.0) as u32);
        if img.texture_descriptor.size.width != ww || img.texture_descriptor.size.height != wh {
            img.resize(Extent3d {
                width: ww,
                height: wh,
                depth_or_array_layers: 1,
            });
        }
    }
    for mut t in crt_q.iter_mut() {
        t.scale = Vec3::new(screen.half_width, screen.half_height, 1.0);
    }
}

fn update_crt_material(
    time: Res<Time>,
    screen: Res<ScreenSize>,
    mut mats: ResMut<Assets<CrtMaterial>>,
    mat_h: Option<Res<CrtMaterialHandle>>,
    crt: Res<CrtState>,
) {
    let Some(h) = mat_h else { return };
    if let Some(mat) = mats.get_mut(&h.0) {
        mat.params = CrtParams {
            time: time.elapsed_secs(),
            width: screen.width,
            height: screen.height,
            crt_enabled: if crt.enabled { 1 } else { 0 },
        };
    }
}

fn crt_toggle(kb: Res<ButtonInput<KeyCode>>, mut crt: ResMut<CrtState>) {
    if kb.just_pressed(KeyCode::KeyC) {
        crt.enabled = !crt.enabled;
    }
}

// ============================================================================
// State Transitions
// ============================================================================

fn hide_title_ui(mut q: Query<(&mut Visibility, &UiMarker)>) {
    for (mut v, m) in q.iter_mut() {
        if matches!(
            m,
            UiMarker::Title | UiMarker::PressEnter | UiMarker::Subtitle
        ) {
            *v = Visibility::Hidden;
        }
    }
}

fn enter_playing(
    mut cmd: Commands,
    mut gd: ResMut<GameData>,
    mut fade: ResMut<MusicFade>,
    screen: Res<ScreenSize>,
    sprites: Res<SpriteAssets>,
    mut uiq: Query<(&mut Visibility, &UiMarker)>,
) {
    request_subsong(&mut fade, 2);
    gd.reset();
    spawn_player(&mut cmd, screen.half_height, &sprites);
    spawn_fading_text(
        &mut cmd,
        "WAVE 1",
        2.0,
        Color::srgba(1.0, 1.0, 0.2, 1.0),
        true,
    );

    // Spawn sprite-based UI elements
    spawn_life_icons(&mut cmd, &sprites, &screen, gd.lives);
    spawn_score_digits(&mut cmd, &sprites, &screen, ScoreType::Score, gd.score);
    spawn_score_digits(
        &mut cmd,
        &sprites,
        &screen,
        ScoreType::HighScore,
        gd.high_score,
    );
    spawn_wave_digits(&mut cmd, &sprites, &screen, gd.wave);

    // Hide all text-based UI elements (now all sprite-based)
    for (mut v, _) in uiq.iter_mut() {
        *v = Visibility::Hidden;
    }
}

fn enter_gameover(
    mut cmd: Commands,
    mut fade: ResMut<MusicFade>,
    mut uiq: Query<(Entity, &mut Visibility, &UiMarker)>,
    eq: Query<(Entity, &Transform), With<Enemy>>,
    player_bullets: Query<Entity, With<PlayerBullet>>,
    enemy_bullets: Query<Entity, With<EnemyBullet>>,
    mut rng: Local<Option<SmallRng>>,
) {
    let rng = rng.get_or_insert_with(SmallRng::from_os_rng);
    request_subsong(&mut fade, 3);

    // Despawn all projectiles
    for e in player_bullets.iter().chain(enemy_bullets.iter()) {
        cmd.entity(e).despawn();
    }

    for (entity, mut v, m) in uiq.iter_mut() {
        if *m == UiMarker::GameOver {
            *v = Visibility::Visible;
            cmd.entity(entity).insert(GameOverUi { base_top: 45.0 });
        }
    }

    let mut enemies: Vec<_> = eq.iter().collect();
    // Fisher-Yates shuffle
    for i in (1..enemies.len()).rev() {
        let j = rng.random_range(0..=i);
        enemies.swap(i, j);
    }
    for (i, (entity, t)) in enemies.into_iter().enumerate() {
        cmd.entity(entity)
            .remove::<DivingEnemy>()
            .insert(GameOverEnemy {
                phase: rng.random_range(0.0..std::f32::consts::TAU),
                amplitude: rng.random_range(80.0..180.0),
                frequency: rng.random_range(0.5..1.5),
                base_pos: t.translation.truncate(),
                delay: i as f32 * 0.08,
                started: false,
            });
    }
}

fn exit_gameover(mut cmd: Commands, mut q: Query<(Entity, &mut Visibility, &UiMarker)>) {
    for (entity, mut v, m) in q.iter_mut() {
        if *m == UiMarker::GameOver {
            *v = Visibility::Hidden;
            cmd.entity(entity).remove::<GameOverUi>();
        }
    }
}

// ============================================================================
// Message Handlers
// ============================================================================

fn handle_sfx_events(
    mut events: MessageReader<PlaySfxMsg>,
    gist: Option<Res<GistRes>>,
    sfx: Option<Res<Sfx>>,
) {
    for ev in events.read() {
        if let (Some(g), Some(s)) = (&gist, &sfx)
            && let Ok(mut p) = g.0.lock()
        {
            p.play_sound(
                match ev.0 {
                    SfxType::Laser => &s.laser,
                    SfxType::Explode => &s.explode,
                    SfxType::Death => &s.death,
                },
                None,
                None,
            );
        }
    }
}

fn handle_wave_complete(
    mut cmd: Commands,
    mut events: MessageReader<WaveCompleteMsg>,
    mut gd: ResMut<GameData>,
) {
    for _ in events.read() {
        gd.wave += 1;
        gd.enemy_direction = 1.0;
        gd.dive_timer = Timer::from_seconds(gd.dive_interval(), TimerMode::Once);
        spawn_fading_text(
            &mut cmd,
            &format!("WAVE {}", gd.wave),
            2.0,
            Color::srgba(1.0, 1.0, 0.2, 1.0),
            true,
        );
    }
}

fn handle_player_hit(
    mut cmd: Commands,
    mut events: MessageReader<PlayerHitMsg>,
    mut gd: ResMut<GameData>,
    mut ns: ResMut<NextState<GameState>>,
    pq: Query<Entity, With<Player>>,
    screen: Res<ScreenSize>,
    sprites: Res<SpriteAssets>,
    mut sfx: MessageWriter<PlaySfxMsg>,
) {
    for _ in events.read() {
        sfx.write(PlaySfxMsg(SfxType::Death));
        gd.lives = gd.lives.saturating_sub(1);
        for e in pq.iter() {
            cmd.entity(e).despawn();
        }
        if gd.lives == 0 {
            ns.set(GameState::GameOver);
        } else {
            spawn_player(&mut cmd, screen.half_height, &sprites);
        }
    }
}

fn handle_enemy_killed(
    mut events: MessageReader<EnemyKilledMsg>,
    mut gd: ResMut<GameData>,
    mut sfx: MessageWriter<PlaySfxMsg>,
    mut extra: MessageWriter<ExtraLifeMsg>,
) {
    for ev in events.read() {
        gd.score += ev.0;
        gd.high_score = gd.high_score.max(gd.score);
        sfx.write(PlaySfxMsg(SfxType::Explode));
        if gd.score >= gd.next_extra_life {
            gd.next_extra_life += EXTRA_LIFE_SCORE;
            extra.write(ExtraLifeMsg);
        }
    }
}

fn handle_extra_life(
    mut cmd: Commands,
    mut events: MessageReader<ExtraLifeMsg>,
    mut gd: ResMut<GameData>,
) {
    for _ in events.read() {
        gd.lives += 1;
        spawn_fading_text(
            &mut cmd,
            "LIVES +1",
            1.0,
            Color::srgba(0.2, 1.0, 0.2, 1.0),
            false,
        );
    }
}

// ============================================================================
// Title Screen
// ============================================================================

fn title_input(kb: Res<ButtonInput<KeyCode>>, mut ns: ResMut<NextState<GameState>>) {
    if kb.just_pressed(KeyCode::Enter) {
        ns.set(GameState::Playing);
    }
    if kb.just_pressed(KeyCode::Escape) {
        std::process::exit(0);
    }
}

fn title_anim(time: Res<Time>, mut q: Query<(&mut TextColor, &UiMarker)>) {
    for (mut c, m) in q.iter_mut() {
        if *m == UiMarker::PressEnter {
            c.0 = Color::srgba(1.0, 1.0, 0.2, (time.elapsed_secs() * 2.0).sin() * 0.3 + 0.7);
        }
    }
}

// ============================================================================
// Gameplay Systems
// ============================================================================

fn fading_text_update(
    mut cmd: Commands,
    time: Res<Time>,
    sprites: Res<SpriteAssets>,
    mut q: Query<(Entity, &mut FadingText, Option<&mut TextColor>)>,
) {
    for (entity, mut ft, color) in q.iter_mut() {
        ft.timer.tick(time.delta());
        if let Some(mut c) = color {
            c.0 = ft.color.with_alpha(1.0 - ft.timer.fraction());
        }
        if ft.timer.is_finished() {
            cmd.entity(entity).despawn();
            if ft.spawn_enemies {
                spawn_enemies(&mut cmd, &sprites);
            }
        }
    }
}

fn player_movement(
    kb: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut q: Query<(&mut Transform, &mut Sprite), With<Player>>,
    screen: Res<ScreenSize>,
) {
    let Ok((mut t, mut sprite)) = q.single_mut() else {
        return;
    };
    let hw = screen.half_width - PLAYER_SIZE.x / 2.0;
    let dir = kb.pressed(KeyCode::ArrowRight) as i32 - kb.pressed(KeyCode::ArrowLeft) as i32;
    t.translation.x =
        (t.translation.x + dir as f32 * PLAYER_SPEED * time.delta_secs()).clamp(-hw, hw);

    // Update sprite based on movement direction: 0=left, 1=center, 2=right
    if let Some(atlas) = &mut sprite.texture_atlas {
        atlas.index = match dir {
            -1 => 0, // Moving left
            1 => 2,  // Moving right
            _ => 1,  // Standing still (center)
        };
    }
}

fn player_shooting(
    mut cmd: Commands,
    kb: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut gd: ResMut<GameData>,
    pq: Query<&Transform, With<Player>>,
    sprites: Res<SpriteAssets>,
    mut sfx: MessageWriter<PlaySfxMsg>,
) {
    gd.shoot_timer.tick(time.delta());
    if kb.pressed(KeyCode::Space)
        && gd.shoot_timer.is_finished()
        && let Ok(pt) = pq.single()
    {
        cmd.spawn((
            Sprite::from_atlas_image(
                sprites.player_bullet_texture.clone(),
                TextureAtlas {
                    layout: sprites.player_bullet_layout.clone(),
                    index: 0,
                },
            ),
            Transform::from_xyz(
                pt.translation.x,
                pt.translation.y + PLAYER_SIZE.y / 2.0,
                1.0,
            )
            .with_scale(Vec3::splat(SPRITE_SCALE * 0.5)),
            PlayerBullet,
            GameEntity,
            AnimationIndices { first: 0, last: 1 },
            AnimationTimer(Timer::from_seconds(0.05, TimerMode::Repeating)),
        ));
        sfx.write(PlaySfxMsg(SfxType::Laser));
        gd.shoot_timer = Timer::from_seconds(0.25, TimerMode::Once);
    }
}

fn bullet_movement(
    mut cmd: Commands,
    time: Res<Time>,
    screen: Res<ScreenSize>,
    mut player_bullets: Query<(Entity, &mut Transform), With<PlayerBullet>>,
    mut enemy_bullets: Query<(Entity, &mut Transform), (With<EnemyBullet>, Without<PlayerBullet>)>,
) {
    let hh = screen.half_height;
    for (e, mut t) in player_bullets.iter_mut() {
        t.translation.y += BULLET_SPEED * time.delta_secs();
        if t.translation.y > hh + 20.0 {
            cmd.entity(e).despawn();
        }
    }
    for (e, mut t) in enemy_bullets.iter_mut() {
        t.translation.y -= ENEMY_BULLET_SPEED * time.delta_secs();
        if t.translation.y < -hh - 20.0 {
            cmd.entity(e).despawn();
        }
    }
}

fn enemy_formation_movement(
    time: Res<Time>,
    mut gd: ResMut<GameData>,
    mut q: Query<&mut Transform, (With<Enemy>, Without<DivingEnemy>)>,
    screen: Res<ScreenSize>,
) {
    let hw = screen.half_width - ENEMY_SIZE.x;
    let edge = q.iter().any(|t| {
        (t.translation.x > hw && gd.enemy_direction > 0.0)
            || (t.translation.x < -hw && gd.enemy_direction < 0.0)
    });
    if edge {
        gd.enemy_direction *= -1.0;
    }
    for mut t in q.iter_mut() {
        t.translation.x += gd.enemy_direction * 50.0 * time.delta_secs();
        if edge {
            t.translation.y -= 20.0;
        }
    }
}

fn enemy_shooting(
    mut cmd: Commands,
    time: Res<Time>,
    mut gd: ResMut<GameData>,
    sprites: Res<SpriteAssets>,
    q: Query<&Transform, With<Enemy>>,
    mut rng: Local<Option<SmallRng>>,
) {
    let rng = rng.get_or_insert_with(SmallRng::from_os_rng);
    gd.enemy_shoot_timer.tick(time.delta());
    if gd.enemy_shoot_timer.is_finished() {
        let enemies: Vec<_> = q.iter().collect();
        if !enemies.is_empty() {
            let t = enemies[rng.random_range(0..enemies.len())];
            // Pick a random bullet variant (0, 1, or 2)
            let bullet_idx = rng.random_range(0..3);
            cmd.spawn((
                Sprite::from_atlas_image(
                    sprites.enemy_bullet_texture.clone(),
                    TextureAtlas {
                        layout: sprites.enemy_bullet_layout.clone(),
                        index: bullet_idx,
                    },
                ),
                Transform::from_xyz(t.translation.x, t.translation.y - ENEMY_SIZE.y / 2.0, 1.0)
                    .with_scale(Vec3::splat(SPRITE_SCALE * 0.5)),
                EnemyBullet,
                GameEntity,
            ));
        }
        gd.enemy_shoot_timer = Timer::from_seconds(gd.enemy_shoot_rate(), TimerMode::Once);
    }
}

fn initiate_dives(
    mut cmd: Commands,
    time: Res<Time>,
    mut gd: ResMut<GameData>,
    eq: Query<(Entity, &Transform), (With<Enemy>, Without<DivingEnemy>)>,
    dq: Query<&DivingEnemy>,
    pq: Query<&Transform, With<Player>>,
    mut rng: Local<Option<SmallRng>>,
) {
    if gd.wave < 2 {
        return;
    }
    let rng = rng.get_or_insert_with(SmallRng::from_os_rng);
    gd.dive_timer.tick(time.delta());
    if gd.dive_timer.is_finished() {
        gd.dive_timer = Timer::from_seconds(gd.dive_interval(), TimerMode::Once);
        let (current, max) = (dq.iter().count(), gd.max_divers());
        if current >= max {
            return;
        }
        let Ok(pt) = pq.single() else { return };
        let candidates: Vec<_> = eq.iter().collect();
        if candidates.is_empty() {
            return;
        }
        for _ in 0..(max - current)
            .min(1 + gd.wave as usize / 3)
            .min(candidates.len())
        {
            let (e, t) = candidates[rng.random_range(0..candidates.len())];
            cmd.entity(e).insert(DivingEnemy {
                target_x: pt.translation.x + rng.random_range(-50.0..50.0),
                returning: false,
                start_y: t.translation.y,
                original_x: t.translation.x,
                progress: 0.0,
                amplitude: rng.random_range(60.0..120.0),
                curve_dir: if rng.random_bool(0.5) { 1.0 } else { -1.0 },
            });
        }
    }
}

fn diving_movement(
    mut cmd: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut Transform, &mut DivingEnemy)>,
    screen: Res<ScreenSize>,
) {
    let bottom = -screen.half_height + 40.0;
    let dt = time.delta_secs();

    for (e, mut t, mut d) in q.iter_mut() {
        if !d.returning {
            d.progress += dt * 0.8;
            let total_drop = d.start_y - bottom;
            let target_y = d.start_y - d.progress * total_drop;
            let wave = (d.progress * std::f32::consts::PI * 2.0).sin();
            let base_x = d.original_x + (d.target_x - d.original_x) * d.progress;
            t.translation.x = base_x + wave * d.amplitude * d.curve_dir;
            t.translation.y = target_y;
            if d.progress >= 1.0 {
                d.returning = true;
                d.progress = 0.0;
            }
        } else {
            d.progress += dt * 0.6;
            let p = d.progress.min(1.0);
            let ease = 1.0 - (1.0 - p).powi(2);
            let arc = (p * std::f32::consts::PI).sin();
            let target_x = d.target_x + (d.original_x - d.target_x) * ease;
            t.translation.x = target_x + arc * d.amplitude * 0.5 * -d.curve_dir;
            t.translation.y = bottom + (d.start_y - bottom) * ease;
            if d.progress >= 1.0 {
                t.translation = Vec3::new(d.original_x, d.start_y, 1.0);
                cmd.entity(e).remove::<DivingEnemy>();
            }
        }
    }
}

// ============================================================================
// Collision System
// ============================================================================

fn collisions(
    mut cmd: Commands,
    player_bullets: Query<(Entity, &Transform), With<PlayerBullet>>,
    enemy_bullets: Query<(Entity, &Transform), With<EnemyBullet>>,
    eq: Query<(Entity, &Transform, &Enemy)>,
    pq: Query<&Transform, With<Player>>,
    dq: Query<(Entity, &Transform), With<DivingEnemy>>,
    sprites: Res<SpriteAssets>,
    mut enemy_killed: MessageWriter<EnemyKilledMsg>,
    mut player_hit: MessageWriter<PlayerHitMsg>,
) {
    // Player bullets vs enemies
    for (be, bt) in player_bullets.iter() {
        let bp = bt.translation.truncate();
        for (ee, et, enemy) in eq.iter() {
            if bp.distance(et.translation.truncate()) < BULLET_SIZE.y / 2.0 + ENEMY_SIZE.x / 2.0 {
                cmd.entity(be).despawn();
                cmd.entity(ee).despawn();
                // Spawn explosion at enemy position
                spawn_explosion(&mut cmd, et.translation, &sprites);
                enemy_killed.write(EnemyKilledMsg(enemy.points));
                break;
            }
        }
    }

    // Enemy bullets and divers vs player
    let Ok(pt) = pq.single() else { return };
    let pp = pt.translation.truncate();

    for (be, bt) in enemy_bullets.iter() {
        if bt.translation.truncate().distance(pp) < BULLET_SIZE.y / 2.0 + PLAYER_SIZE.x / 2.0 {
            cmd.entity(be).despawn();
            player_hit.write(PlayerHitMsg);
            return;
        }
    }

    for (de, dt) in dq.iter() {
        if dt.translation.truncate().distance(pp) < ENEMY_SIZE.x / 2.0 + PLAYER_SIZE.x / 2.0 {
            cmd.entity(de).despawn();
            // Spawn explosion when diving enemy hits player
            spawn_explosion(&mut cmd, dt.translation, &sprites);
            player_hit.write(PlayerHitMsg);
            return;
        }
    }
}

fn spawn_explosion(cmd: &mut Commands, pos: Vec3, sprites: &SpriteAssets) {
    cmd.spawn((
        Sprite::from_atlas_image(
            sprites.explosion_texture.clone(),
            TextureAtlas {
                layout: sprites.explosion_layout.clone(),
                index: 0,
            },
        ),
        Transform::from_translation(pos).with_scale(Vec3::splat(SPRITE_SCALE)),
        Explosion,
        GameEntity,
        AnimationIndices { first: 0, last: 4 },
        AnimationTimer(Timer::from_seconds(0.08, TimerMode::Repeating)),
    ));
}

fn check_wave_complete(
    eq: Query<Entity, With<Enemy>>,
    aq: Query<&FadingText>,
    mut events: MessageWriter<WaveCompleteMsg>,
) {
    if eq.is_empty() && aq.is_empty() {
        events.write(WaveCompleteMsg);
    }
}

// ============================================================================
// Animation Systems
// ============================================================================

/// Animate all sprites with AnimationIndices and AnimationTimer components
fn animate_sprites(
    time: Res<Time>,
    mut q: Query<(&AnimationIndices, &mut AnimationTimer, &mut Sprite)>,
) {
    for (indices, mut timer, mut sprite) in q.iter_mut() {
        timer.tick(time.delta());
        if timer.just_finished() {
            if let Some(atlas) = &mut sprite.texture_atlas {
                atlas.index = if atlas.index >= indices.last {
                    indices.first
                } else {
                    atlas.index + 1
                };
            }
        }
    }
}

/// Update explosions - despawn when animation completes one cycle
fn explosion_update(
    mut cmd: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &AnimationIndices, &mut AnimationTimer, &Sprite), With<Explosion>>,
) {
    for (entity, indices, mut timer, sprite) in q.iter_mut() {
        timer.tick(time.delta());
        if timer.just_finished() {
            if let Some(atlas) = &sprite.texture_atlas {
                // If we've reached the last frame, despawn the explosion
                if atlas.index >= indices.last {
                    cmd.entity(entity).despawn();
                }
            }
        }
    }
}

// ============================================================================
// Global Systems
// ============================================================================

fn starfield(time: Res<Time>, mut q: Query<(&mut Transform, &Star)>, screen: Res<ScreenSize>) {
    let hh = screen.half_height;
    for (mut t, s) in q.iter_mut() {
        t.translation.y -= s.speed * time.delta_secs();
        if t.translation.y < -hh {
            t.translation.y = hh;
        }
    }
}

fn update_ui(gd: Res<GameData>, mut q: Query<(&mut Text, &UiMarker)>) {
    for (mut t, m) in q.iter_mut() {
        // Only update Wave text - Score/High/Lives are sprite-based now
        if *m == UiMarker::Wave {
            t.0 = format!("WAVE {}", gd.wave);
        }
    }
}

/// Update life icon sprites when lives count changes
fn update_life_icons(
    mut cmd: Commands,
    gd: Res<GameData>,
    sprites: Res<SpriteAssets>,
    screen: Res<ScreenSize>,
    life_icons: Query<Entity, With<LifeIcon>>,
) {
    let current_count = life_icons.iter().count() as u32;

    if current_count != gd.lives {
        // Despawn all existing life icons
        for entity in life_icons.iter() {
            cmd.entity(entity).despawn();
        }
        // Respawn with correct count
        spawn_life_icons(&mut cmd, &sprites, &screen, gd.lives);
    }
}

/// Update score digit sprites when score changes
fn update_score_digits(
    gd: Res<GameData>,
    mut score_digits: Query<(&mut Sprite, &DigitSprite, &ScoreType)>,
) {
    // Get score and high_score as 6-digit strings
    let score_str = format!("{:06}", gd.score.min(999999));
    let high_str = format!("{:06}", gd.high_score.min(999999));

    for (mut sprite, digit_sprite, score_type) in score_digits.iter_mut() {
        let value_str = match score_type {
            ScoreType::Score => &score_str,
            ScoreType::HighScore => &high_str,
        };

        if let Some(digit_char) = value_str.chars().nth(digit_sprite.position) {
            if let Some(atlas) = &mut sprite.texture_atlas {
                let digit = digit_char.to_digit(10).unwrap_or(0) as u8;
                atlas.index = digit_to_atlas_index(digit);
            }
        }
    }
}

/// Update wave digit sprites when wave changes
fn update_wave_digits(
    mut cmd: Commands,
    gd: Res<GameData>,
    sprites: Res<SpriteAssets>,
    screen: Res<ScreenSize>,
    wave_digits: Query<Entity, With<WaveDigit>>,
) {
    // Respawn wave digits when wave changes
    // (simpler than tracking individual digit positions for 2 digits)
    let current_digits: Vec<_> = wave_digits.iter().collect();
    if current_digits.len() != 2 {
        return; // Not yet spawned or something weird
    }

    // Despawn and respawn on wave change
    for entity in current_digits {
        cmd.entity(entity).despawn();
    }
    spawn_wave_digits(&mut cmd, &sprites, &screen, gd.wave);
}

fn game_input(
    mut cmd: Commands,
    kb: Res<ButtonInput<KeyCode>>,
    mut gd: ResMut<GameData>,
    mut ns: ResMut<NextState<GameState>>,
    state: Res<State<GameState>>,
    eq: Query<Entity, With<Enemy>>,
    player_bullets: Query<Entity, With<PlayerBullet>>,
    enemy_bullets: Query<Entity, With<EnemyBullet>>,
    pq: Query<Entity, With<Player>>,
    aq: Query<Entity, With<FadingText>>,
    life_icons: Query<Entity, With<LifeIcon>>,
    digit_sprites: Query<Entity, With<DigitSprite>>,
    wave_digits: Query<Entity, With<WaveDigit>>,
    screen: Res<ScreenSize>,
    sprites: Res<SpriteAssets>,
    mut fade: ResMut<MusicFade>,
) {
    if kb.just_pressed(KeyCode::KeyR) {
        for e in eq
            .iter()
            .chain(player_bullets.iter())
            .chain(enemy_bullets.iter())
            .chain(aq.iter())
            .chain(life_icons.iter())
            .chain(digit_sprites.iter())
            .chain(wave_digits.iter())
        {
            cmd.entity(e).despawn();
        }
        // Player has child booster, so despawn recursively
        for e in pq.iter() {
            cmd.entity(e).despawn();
        }
        gd.reset();
        spawn_player(&mut cmd, screen.half_height, &sprites);
        spawn_fading_text(
            &mut cmd,
            "WAVE 1",
            2.0,
            Color::srgba(1.0, 1.0, 0.2, 1.0),
            true,
        );

        // Respawn sprite UI
        spawn_life_icons(&mut cmd, &sprites, &screen, gd.lives);
        spawn_score_digits(&mut cmd, &sprites, &screen, ScoreType::Score, gd.score);
        spawn_score_digits(
            &mut cmd,
            &sprites,
            &screen,
            ScoreType::HighScore,
            gd.high_score,
        );
        spawn_wave_digits(&mut cmd, &sprites, &screen, gd.wave);

        request_subsong(&mut fade, 2);
        if *state.get() == GameState::GameOver {
            ns.set(GameState::Playing);
        }
    }
    if kb.just_pressed(KeyCode::Escape) {
        std::process::exit(0);
    }
}

fn music_toggle(kb: Res<ButtonInput<KeyCode>>, mut q: Query<&mut Ym2149Playback>) {
    if kb.just_pressed(KeyCode::KeyM)
        && let Ok(mut p) = q.single_mut()
    {
        if p.state == bevy_ym2149::PlaybackState::Playing {
            p.pause();
        } else {
            p.play();
        }
    }
}

// ============================================================================
// Music Crossfade
// ============================================================================

fn request_subsong(fade: &mut MusicFade, subsong: usize) {
    fade.target_subsong = Some(subsong);
    fade.phase = FadePhase::FadeOut;
    fade.timer = 0.0;
}

fn music_crossfade(
    time: Res<Time>,
    mut fade: ResMut<MusicFade>,
    mut q: Query<&mut Ym2149Playback>,
) {
    if fade.phase == FadePhase::Idle {
        return;
    }
    let Ok(mut p) = q.single_mut() else { return };

    // Skip if already on target subsong
    if let Some(target) = fade.target_subsong
        && p.current_subsong() == target
        && fade.phase == FadePhase::FadeOut
        && fade.timer == 0.0
    {
        fade.phase = FadePhase::Idle;
        fade.target_subsong = None;
        return;
    }

    fade.timer += time.delta_secs();
    let progress = (fade.timer / FADE_DURATION).min(1.0);

    match fade.phase {
        FadePhase::FadeOut => {
            p.set_volume(1.0 - progress);
            if progress >= 1.0 {
                if let Some(subsong) = fade.target_subsong {
                    p.set_subsong(subsong);
                    p.play();
                }
                fade.phase = FadePhase::FadeIn;
                fade.timer = 0.0;
            }
        }
        FadePhase::FadeIn => {
            p.set_volume(progress);
            if progress >= 1.0 {
                fade.phase = FadePhase::Idle;
                fade.target_subsong = None;
            }
        }
        FadePhase::Idle => {}
    }
}

// ============================================================================
// Game Over Animations
// ============================================================================

fn gameover_enemy_movement(
    time: Res<Time>,
    mut q: Query<(&mut Transform, &mut GameOverEnemy)>,
    screen: Res<ScreenSize>,
) {
    let (hw, hh) = (screen.half_width - 40.0, screen.half_height - 40.0);
    let t = time.elapsed_secs();
    let dt = time.delta_secs();

    for (mut tr, mut ge) in q.iter_mut() {
        if !ge.started {
            ge.delay -= dt;
            if ge.delay <= 0.0 {
                ge.started = true;
                ge.phase = t;
            }
            continue;
        }
        let local_t = (t - ge.phase) * ge.frequency;
        let blend = ((t - ge.phase) / 2.0).min(1.0);
        let target_x = (local_t + ge.amplitude).sin() * hw;
        let target_y = (local_t * 0.7 + ge.amplitude * 0.5).sin() * hh;
        tr.translation.x = ge.base_pos.x + (target_x - ge.base_pos.x) * blend;
        tr.translation.y = ge.base_pos.y + (target_y - ge.base_pos.y) * blend;
    }
}

fn gameover_ui_animation(time: Res<Time>, mut q: Query<(&mut Node, &GameOverUi)>) {
    let t = time.elapsed_secs();
    for (mut node, ui) in q.iter_mut() {
        let offset = (t * 1.5).sin() * 1.5;
        node.top = Val::Percent(ui.base_top + offset);
    }
}
