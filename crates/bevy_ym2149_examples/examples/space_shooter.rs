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
use rand::Rng;
use std::sync::{Arc, Mutex};
use ym2149_gist_replayer::{GistPlayer, GistSound};

// Constants
const PLAYER_SPEED: f32 = 400.0;
const PLAYER_SIZE: Vec2 = Vec2::new(40.0, 30.0);
const BULLET_SPEED: f32 = 600.0;
const BULLET_SIZE: Vec2 = Vec2::new(4.0, 12.0);
const ENEMY_SIZE: Vec2 = Vec2::new(32.0, 24.0);
const ENEMY_BULLET_SPEED: f32 = 300.0;
const ENEMY_SPACING: Vec2 = Vec2::new(50.0, 40.0);
const STARTING_LIVES: u32 = 3;
const DIVE_SPEED: f32 = 250.0;
const WAVE_ANNOUNCE_DURATION: f32 = 2.0;

#[derive(States, Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
enum GameState {
    #[default]
    TitleScreen,
    Playing,
    GameOver,
}

// Messages for decoupled ECS communication
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

const EXTRA_LIFE_SCORE: u32 = 3000;

#[derive(Resource)]
struct GameData {
    score: u32,
    high_score: u32,
    lives: u32,
    wave: u32,
    shoot_timer: f32,
    enemy_shoot_timer: f32,
    enemy_direction: f32,
    dive_timer: f32,
    next_extra_life: u32,
}

impl Default for GameData {
    fn default() -> Self {
        Self {
            score: 0,
            high_score: 0,
            lives: STARTING_LIVES,
            wave: 1,
            shoot_timer: 0.0,
            enemy_shoot_timer: 1.5,
            enemy_direction: 1.0,
            dive_timer: 3.0,
            next_extra_life: EXTRA_LIFE_SCORE,
        }
    }
}

impl GameData {
    fn reset(&mut self) {
        let hs = self.high_score;
        *self = Self::default();
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

// Components
#[derive(Component)]
struct Player;
#[derive(Component)]
struct Bullet {
    from_player: bool,
}
#[derive(Component)]
struct Enemy {
    points: u32,
}
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
}
#[derive(Component)]
struct WaveAnnouncement {
    timer: Timer,
}
#[derive(Component)]
struct ExtraLifeNotification {
    timer: Timer,
}
#[derive(Component)]
struct GameEntity; // Marker for game entities (rendered to offscreen target)

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

// CRT Material
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

#[derive(Resource)]
struct CrtState {
    enabled: bool,
}
#[derive(Resource)]
struct CrtMaterialHandle(Handle<CrtMaterial>);
#[derive(Resource)]
struct SceneRenderTarget(Handle<Image>);
#[derive(Component)]
struct CrtQuad;

// Audio resources
#[derive(Resource)]
struct Sfx {
    laser: GistSound,
    explode: GistSound,
    death: GistSound,
}
#[derive(Resource, Clone)]
struct GistRes(Arc<Mutex<GistPlayer>>);

#[derive(Asset, TypePath, Clone)]
struct GistAudio {
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

struct GistDec {
    player: Arc<Mutex<GistPlayer>>,
    volume: f32,
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
        .init_resource::<GameData>()
        .insert_resource(CrtState { enabled: true })
        .add_message::<PlaySfxMsg>()
        .add_message::<WaveCompleteMsg>()
        .add_message::<PlayerHitMsg>()
        .add_message::<EnemyKilledMsg>()
        .add_message::<ExtraLifeMsg>()
        .add_systems(Startup, setup)
        // Title
        .add_systems(
            Update,
            (title_input, starfield, title_anim).run_if(in_state(GameState::TitleScreen)),
        )
        .add_systems(
            OnEnter(GameState::TitleScreen),
            |mut q: Query<&mut Ym2149Playback>| {
                if let Ok(mut p) = q.single_mut() {
                    p.set_subsong(1);
                    p.play();
                }
            },
        )
        .add_systems(OnExit(GameState::TitleScreen), hide_title_ui)
        // Playing
        .add_systems(
            Update,
            (
                player_movement,
                player_shooting,
                bullet_movement,
                enemy_formation_movement,
                enemy_shooting,
                diving_movement,
                initiate_dives,
                wave_announcement_update,
                bullet_enemy_collision,
                bullet_player_collision,
            )
                .run_if(in_state(GameState::Playing)),
        )
        .add_systems(
            Update,
            (
                diving_player_collision,
                check_wave_complete,
                update_ui,
                starfield,
                game_input,
                music_toggle,
                crt_toggle,
                handle_sfx_events,
                handle_wave_complete,
                handle_player_hit,
                handle_enemy_killed,
                handle_extra_life,
                extra_life_notification_update,
            )
                .run_if(in_state(GameState::Playing)),
        )
        .add_systems(OnEnter(GameState::Playing), enter_playing)
        // Game Over
        .add_systems(
            Update,
            (game_input, update_ui, starfield, music_toggle, crt_toggle)
                .run_if(in_state(GameState::GameOver)),
        )
        .add_systems(OnEnter(GameState::GameOver), enter_gameover)
        .add_systems(
            OnExit(GameState::GameOver),
            |mut q: Query<(&mut Visibility, &UiMarker)>| {
                for (mut v, m) in q.iter_mut() {
                    if *m == UiMarker::GameOver {
                        *v = Visibility::Hidden;
                    }
                }
            },
        )
        // CRT update (always running)
        .add_systems(Update, (update_crt_material, sync_render_target))
        .run();
}

fn setup(
    mut cmd: Commands,
    mut assets: ResMut<Assets<GistAudio>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut crt_materials: ResMut<Assets<CrtMaterial>>,
    server: Res<AssetServer>,
    wq: Query<&Window, With<PrimaryWindow>>,
) {
    let w = wq.single().unwrap();
    let (ww, wh) = (w.width(), w.height());
    let (hw, hh) = (ww / 2.0, wh / 2.0);

    // Create offscreen render target
    let extent = Extent3d {
        width: ww.max(1.0) as u32,
        height: wh.max(1.0) as u32,
        depth_or_array_layers: 1,
    };
    let mut render_target_image = Image {
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
    render_target_image.resize(extent);
    let render_target = images.add(render_target_image);
    cmd.insert_resource(SceneRenderTarget(render_target.clone()));

    // Game camera (renders to texture)
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

    // Display camera (renders CRT quad to screen)
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

    // CRT quad
    let quad_mesh = meshes.add(Mesh::from(Rectangle::new(2.0, 2.0)));
    let crt_mat = crt_materials.add(CrtMaterial {
        scene_texture: render_target.clone(),
        params: CrtParams {
            time: 0.0,
            width: ww,
            height: wh,
            crt_enabled: 1,
        },
    });
    cmd.insert_resource(CrtMaterialHandle(crt_mat.clone()));
    cmd.spawn((
        Mesh2d(quad_mesh),
        MeshMaterial2d(crt_mat),
        Transform::from_scale(Vec3::new(hw, hh, 1.0)),
        CrtQuad,
        Name::new("CrtQuad"),
    ));

    // Music
    let mut playback = Ym2149Playback::from_asset(server.load("sndh/Lethal_Xcess_(STe).sndh"));
    playback.set_volume(1.0);
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

    // Starfield (rendered to game camera)
    let mut rng = rand::rng();
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

    // Game UI (hidden initially)
    spawn_text(
        &mut cmd,
        "SCORE: 0",
        24.0,
        Color::WHITE,
        Val::Px(15.0),
        Some(Val::Px(20.0)),
        None,
        false,
        UiMarker::Score,
    );
    spawn_text(
        &mut cmd,
        "HIGH: 0",
        24.0,
        Color::srgb(1.0, 0.8, 0.0),
        Val::Px(15.0),
        None,
        None,
        false,
        UiMarker::High,
    );
    spawn_text(
        &mut cmd,
        "LIVES: 3",
        24.0,
        Color::srgb(0.2, 1.0, 0.2),
        Val::Px(15.0),
        None,
        Some(Val::Px(20.0)),
        false,
        UiMarker::Lives,
    );
    spawn_text(
        &mut cmd,
        "WAVE 1",
        18.0,
        Color::srgb(0.6, 0.6, 1.0),
        Val::Px(45.0),
        Some(Val::Px(20.0)),
        None,
        false,
        UiMarker::Wave,
    );
    spawn_text(
        &mut cmd,
        "GAME OVER\n\nPress R to Restart",
        48.0,
        Color::srgb(1.0, 0.2, 0.2),
        Val::Percent(45.0),
        None,
        None,
        false,
        UiMarker::GameOver,
    );

    // Title UI (visible)
    spawn_text(
        &mut cmd,
        "SPACE SHOOTER",
        64.0,
        Color::srgb(0.2, 0.8, 1.0),
        Val::Percent(25.0),
        None,
        None,
        true,
        UiMarker::Title,
    );
    spawn_text(
        &mut cmd,
        "Press ENTER to Start",
        32.0,
        Color::srgb(1.0, 1.0, 0.2),
        Val::Percent(50.0),
        None,
        None,
        true,
        UiMarker::PressEnter,
    );
    spawn_text(
        &mut cmd,
        "Music: Mad Max - Lethal Xcess (STe) | C: Toggle CRT",
        18.0,
        Color::srgba(0.7, 0.7, 0.7, 0.9),
        Val::Percent(65.0),
        None,
        None,
        true,
        UiMarker::Subtitle,
    );
}

#[allow(clippy::too_many_arguments)]
fn spawn_text(
    cmd: &mut Commands,
    txt: &str,
    size: f32,
    color: Color,
    top: Val,
    left: Option<Val>,
    right: Option<Val>,
    visible: bool,
    marker: UiMarker,
) {
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
        if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        },
        marker,
    ));
}

fn spawn_player(cmd: &mut Commands, half_height: f32) {
    cmd.spawn((
        Sprite {
            color: Color::srgb(0.2, 0.8, 0.2),
            custom_size: Some(PLAYER_SIZE),
            ..default()
        },
        Transform::from_xyz(0.0, -half_height + 60.0, 1.0),
        Player,
        GameEntity,
    ));
}

fn spawn_enemies(cmd: &mut Commands) {
    let start_x = -(7.0 * ENEMY_SPACING.x) / 2.0;
    for row in 0..4 {
        let (pts, color) = [
            (40, Color::srgb(1.0, 0.2, 0.2)),
            (30, Color::srgb(1.0, 0.6, 0.2)),
            (20, Color::srgb(1.0, 1.0, 0.2)),
            (10, Color::srgb(0.2, 0.6, 1.0)),
        ][row];
        for col in 0..8 {
            cmd.spawn((
                Sprite {
                    color,
                    custom_size: Some(ENEMY_SIZE),
                    ..default()
                },
                Transform::from_xyz(
                    start_x + col as f32 * ENEMY_SPACING.x,
                    200.0 - row as f32 * ENEMY_SPACING.y,
                    1.0,
                ),
                Enemy { points: pts },
                GameEntity,
            ));
        }
    }
}

fn spawn_wave_announcement(cmd: &mut Commands, wave: u32) {
    cmd.spawn((
        Sprite {
            color: Color::NONE,
            custom_size: Some(Vec2::ZERO),
            ..default()
        },
        Transform::default(),
        WaveAnnouncement {
            timer: Timer::from_seconds(WAVE_ANNOUNCE_DURATION, TimerMode::Once),
        },
        GameEntity,
    ));
    // Text shown via UI (rendered by display camera)
    cmd.spawn((
        Text::new(format!("WAVE {}", wave)),
        TextFont {
            font_size: 72.0,
            ..default()
        },
        TextColor(Color::srgba(1.0, 1.0, 0.2, 1.0)),
        TextLayout::new_with_justify(bevy::text::Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(45.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        WaveAnnouncement {
            timer: Timer::from_seconds(WAVE_ANNOUNCE_DURATION, TimerMode::Once),
        },
    ));
}

// CRT systems
fn sync_render_target(
    wq: Query<&Window, With<PrimaryWindow>>,
    mut images: ResMut<Assets<Image>>,
    rt: Option<Res<SceneRenderTarget>>,
    mut crt_q: Query<&mut Transform, With<CrtQuad>>,
) {
    let Some(rt) = rt else { return };
    let Ok(w) = wq.single() else { return };
    let Some(img) = images.get_mut(&rt.0) else {
        return;
    };

    let (ww, wh) = (w.width().max(1.0) as u32, w.height().max(1.0) as u32);
    let size = img.texture_descriptor.size;
    if size.width != ww || size.height != wh {
        img.resize(Extent3d {
            width: ww,
            height: wh,
            depth_or_array_layers: 1,
        });
    }

    // Update quad scale
    for mut t in crt_q.iter_mut() {
        t.scale = Vec3::new(w.width() / 2.0, w.height() / 2.0, 1.0);
    }
}

fn update_crt_material(
    time: Res<Time>,
    wq: Query<&Window, With<PrimaryWindow>>,
    mut mats: ResMut<Assets<CrtMaterial>>,
    mat_h: Option<Res<CrtMaterialHandle>>,
    crt: Res<CrtState>,
) {
    let Some(h) = mat_h else { return };
    let Ok(w) = wq.single() else { return };
    let Some(mat) = mats.get_mut(&h.0) else {
        return;
    };

    mat.params.time = time.elapsed_secs();
    mat.params.width = w.width();
    mat.params.height = w.height();
    mat.params.crt_enabled = if crt.enabled { 1 } else { 0 };
}

fn crt_toggle(kb: Res<ButtonInput<KeyCode>>, mut crt: ResMut<CrtState>) {
    if kb.just_pressed(KeyCode::KeyC) {
        crt.enabled = !crt.enabled;
    }
}

// State transitions
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
    mut pq: Query<&mut Ym2149Playback>,
    wq: Query<&Window, With<PrimaryWindow>>,
    mut uiq: Query<(&mut Visibility, &UiMarker)>,
) {
    if let Ok(mut p) = pq.single_mut() {
        p.set_subsong(2);
        p.play();
    }
    gd.reset();
    let hh = wq.single().unwrap().height() / 2.0;
    spawn_player(&mut cmd, hh);
    spawn_wave_announcement(&mut cmd, 1);
    for (mut v, m) in uiq.iter_mut() {
        *v = if matches!(
            m,
            UiMarker::Score | UiMarker::High | UiMarker::Lives | UiMarker::Wave
        ) {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

fn enter_gameover(mut pq: Query<&mut Ym2149Playback>, mut q: Query<(&mut Visibility, &UiMarker)>) {
    if let Ok(mut p) = pq.single_mut() {
        p.set_subsong(3);
        p.play();
    }
    for (mut v, m) in q.iter_mut() {
        if *m == UiMarker::GameOver {
            *v = Visibility::Visible;
        }
    }
}

// Message handlers
fn handle_sfx_events(
    mut events: MessageReader<PlaySfxMsg>,
    gist: Option<Res<GistRes>>,
    sfx: Option<Res<Sfx>>,
) {
    for ev in events.read() {
        if let (Some(g), Some(s)) = (&gist, &sfx)
            && let Ok(mut p) = g.0.lock()
        {
            let sound = match ev.0 {
                SfxType::Laser => &s.laser,
                SfxType::Explode => &s.explode,
                SfxType::Death => &s.death,
            };
            p.play_sound(sound, None, None);
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
        gd.dive_timer = gd.dive_interval();
        spawn_wave_announcement(&mut cmd, gd.wave);
    }
}

fn handle_player_hit(
    mut cmd: Commands,
    mut events: MessageReader<PlayerHitMsg>,
    mut gd: ResMut<GameData>,
    mut ns: ResMut<NextState<GameState>>,
    pq: Query<Entity, With<Player>>,
    wq: Query<&Window, With<PrimaryWindow>>,
    mut sfx_events: MessageWriter<PlaySfxMsg>,
) {
    for _ in events.read() {
        sfx_events.write(PlaySfxMsg(SfxType::Death));
        gd.lives = gd.lives.saturating_sub(1);
        for e in pq.iter() {
            cmd.entity(e).despawn();
        }
        if gd.lives == 0 {
            ns.set(GameState::GameOver);
        } else {
            spawn_player(&mut cmd, wq.single().unwrap().height() / 2.0);
        }
    }
}

fn handle_enemy_killed(
    mut events: MessageReader<EnemyKilledMsg>,
    mut gd: ResMut<GameData>,
    mut sfx: MessageWriter<PlaySfxMsg>,
    mut extra_life: MessageWriter<ExtraLifeMsg>,
) {
    for ev in events.read() {
        gd.score += ev.0;
        gd.high_score = gd.high_score.max(gd.score);
        sfx.write(PlaySfxMsg(SfxType::Explode));
        // Check for extra life
        if gd.score >= gd.next_extra_life {
            gd.next_extra_life += EXTRA_LIFE_SCORE;
            extra_life.write(ExtraLifeMsg);
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
        // Spawn notification UI
        cmd.spawn((
            Text::new("LIVES +1"),
            TextFont {
                font_size: 48.0,
                ..default()
            },
            TextColor(Color::srgba(0.2, 1.0, 0.2, 1.0)),
            TextLayout::new_with_justify(bevy::text::Justify::Center),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(40.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            ExtraLifeNotification {
                timer: Timer::from_seconds(1.0, TimerMode::Once),
            },
        ));
    }
}

fn extra_life_notification_update(
    mut cmd: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut ExtraLifeNotification, &mut TextColor)>,
) {
    for (entity, mut notif, mut color) in q.iter_mut() {
        notif.timer.tick(time.delta());
        let alpha = 1.0 - notif.timer.fraction();
        color.0 = Color::srgba(0.2, 1.0, 0.2, alpha);
        if notif.timer.is_finished() {
            cmd.entity(entity).despawn();
        }
    }
}

// Title screen
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

// Wave announcement
fn wave_announcement_update(
    mut cmd: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut WaveAnnouncement, Option<&mut TextColor>)>,
) {
    for (entity, mut ann, color) in q.iter_mut() {
        ann.timer.tick(time.delta());
        let is_ui = color.is_some();
        if let Some(mut c) = color {
            let alpha = 1.0 - ann.timer.fraction();
            c.0 = Color::srgba(1.0, 1.0, 0.2, alpha);
        }
        if ann.timer.is_finished() {
            cmd.entity(entity).despawn();
            // Only spawn enemies from the GameEntity marker (non-UI)
            if !is_ui {
                spawn_enemies(&mut cmd);
            }
        }
    }
}

// Player systems
fn player_movement(
    kb: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut q: Query<&mut Transform, With<Player>>,
    wq: Query<&Window, With<PrimaryWindow>>,
) {
    let Ok(mut t) = q.single_mut() else { return };
    let hw = wq.single().unwrap().width() / 2.0 - PLAYER_SIZE.x / 2.0;
    let dir = kb.pressed(KeyCode::ArrowRight) as i32 - kb.pressed(KeyCode::ArrowLeft) as i32;
    t.translation.x =
        (t.translation.x + dir as f32 * PLAYER_SPEED * time.delta_secs()).clamp(-hw, hw);
}

fn player_shooting(
    mut cmd: Commands,
    kb: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut gd: ResMut<GameData>,
    pq: Query<&Transform, With<Player>>,
    mut sfx: MessageWriter<PlaySfxMsg>,
) {
    gd.shoot_timer -= time.delta_secs();
    if kb.pressed(KeyCode::Space)
        && gd.shoot_timer <= 0.0
        && let Ok(pt) = pq.single()
    {
        cmd.spawn((
            Sprite {
                color: Color::srgb(1.0, 1.0, 0.2),
                custom_size: Some(BULLET_SIZE),
                ..default()
            },
            Transform::from_xyz(
                pt.translation.x,
                pt.translation.y + PLAYER_SIZE.y / 2.0,
                1.0,
            ),
            Bullet { from_player: true },
            GameEntity,
        ));
        sfx.write(PlaySfxMsg(SfxType::Laser));
        gd.shoot_timer = 0.25;
    }
}

// Bullet systems
fn bullet_movement(
    mut cmd: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut Transform, &Bullet)>,
    wq: Query<&Window, With<PrimaryWindow>>,
) {
    let hh = wq.single().unwrap().height() / 2.0;
    for (e, mut t, b) in q.iter_mut() {
        t.translation.y += (if b.from_player {
            BULLET_SPEED
        } else {
            -ENEMY_BULLET_SPEED
        }) * time.delta_secs();
        if t.translation.y.abs() > hh + 20.0 {
            cmd.entity(e).despawn();
        }
    }
}

// Enemy systems
fn enemy_formation_movement(
    time: Res<Time>,
    mut gd: ResMut<GameData>,
    mut q: Query<&mut Transform, (With<Enemy>, Without<DivingEnemy>)>,
    wq: Query<&Window, With<PrimaryWindow>>,
) {
    let hw = wq.single().unwrap().width() / 2.0 - ENEMY_SIZE.x;
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
    q: Query<&Transform, With<Enemy>>,
) {
    gd.enemy_shoot_timer -= time.delta_secs();
    if gd.enemy_shoot_timer <= 0.0 {
        let enemies: Vec<_> = q.iter().collect();
        if !enemies.is_empty() {
            let t = enemies[rand::rng().random_range(0..enemies.len())];
            cmd.spawn((
                Sprite {
                    color: Color::srgb(1.0, 0.3, 0.3),
                    custom_size: Some(BULLET_SIZE),
                    ..default()
                },
                Transform::from_xyz(t.translation.x, t.translation.y - ENEMY_SIZE.y / 2.0, 1.0),
                Bullet { from_player: false },
                GameEntity,
            ));
        }
        gd.enemy_shoot_timer = gd.enemy_shoot_rate();
    }
}

#[allow(clippy::type_complexity)]
fn initiate_dives(
    mut cmd: Commands,
    time: Res<Time>,
    mut gd: ResMut<GameData>,
    eq: Query<(Entity, &Transform), (With<Enemy>, Without<DivingEnemy>)>,
    dq: Query<&DivingEnemy>,
    pq: Query<&Transform, With<Player>>,
) {
    if gd.wave < 2 {
        return;
    }
    gd.dive_timer -= time.delta_secs();
    if gd.dive_timer <= 0.0 {
        gd.dive_timer = gd.dive_interval();
        let current_divers = dq.iter().count();
        let max_divers = gd.max_divers();
        if current_divers >= max_divers {
            return;
        }
        let Ok(pt) = pq.single() else { return };
        let candidates: Vec<_> = eq.iter().collect();
        if candidates.is_empty() {
            return;
        }
        let new_divers = (max_divers - current_divers).min(1 + gd.wave as usize / 3);
        let mut rng = rand::rng();
        for _ in 0..new_divers.min(candidates.len()) {
            let idx = rng.random_range(0..candidates.len());
            let (e, t) = candidates[idx];
            cmd.entity(e).insert(DivingEnemy {
                target_x: pt.translation.x + rng.random_range(-50.0..50.0),
                returning: false,
                start_y: t.translation.y,
                original_x: t.translation.x,
            });
        }
    }
}

fn diving_movement(
    mut cmd: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut Transform, &mut DivingEnemy)>,
    wq: Query<&Window, With<PrimaryWindow>>,
) {
    let bottom = -wq.single().unwrap().height() / 2.0 + 80.0;
    let dt = time.delta_secs();
    for (e, mut t, mut d) in q.iter_mut() {
        if !d.returning {
            let dx = d.target_x - t.translation.x;
            t.translation.x += (dx.signum() * DIVE_SPEED * 0.5 * dt).clamp(-dx.abs(), dx.abs());
            t.translation.y -= DIVE_SPEED * dt;
            if t.translation.y <= bottom {
                d.returning = true;
            }
        } else {
            let (dx, dy) = (d.original_x - t.translation.x, d.start_y - t.translation.y);
            t.translation.x += (dx.signum() * DIVE_SPEED * 0.5 * dt).clamp(-dx.abs(), dx.abs());
            t.translation.y += (DIVE_SPEED * dt).min(dy);
            if t.translation.y >= d.start_y {
                t.translation = Vec3::new(d.original_x, d.start_y, 1.0);
                cmd.entity(e).remove::<DivingEnemy>();
            }
        }
    }
}

// Collision systems
fn bullet_enemy_collision(
    mut cmd: Commands,
    bq: Query<(Entity, &Transform, &Bullet)>,
    eq: Query<(Entity, &Transform, &Enemy)>,
    mut events: MessageWriter<EnemyKilledMsg>,
) {
    for (be, bt, b) in bq.iter() {
        if !b.from_player {
            continue;
        }
        for (ee, et, enemy) in eq.iter() {
            if bt
                .translation
                .truncate()
                .distance(et.translation.truncate())
                < BULLET_SIZE.y / 2.0 + ENEMY_SIZE.x / 2.0
            {
                cmd.entity(be).despawn();
                cmd.entity(ee).despawn();
                events.write(EnemyKilledMsg(enemy.points));
                break;
            }
        }
    }
}

fn bullet_player_collision(
    mut cmd: Commands,
    bq: Query<(Entity, &Transform, &Bullet)>,
    pq: Query<&Transform, With<Player>>,
    mut events: MessageWriter<PlayerHitMsg>,
) {
    let Ok(pt) = pq.single() else { return };
    for (be, bt, b) in bq.iter() {
        if !b.from_player
            && bt
                .translation
                .truncate()
                .distance(pt.translation.truncate())
                < BULLET_SIZE.y / 2.0 + PLAYER_SIZE.x / 2.0
        {
            cmd.entity(be).despawn();
            events.write(PlayerHitMsg);
            break;
        }
    }
}

fn diving_player_collision(
    mut cmd: Commands,
    dq: Query<(Entity, &Transform), With<DivingEnemy>>,
    pq: Query<&Transform, With<Player>>,
    mut events: MessageWriter<PlayerHitMsg>,
) {
    let Ok(pt) = pq.single() else { return };
    for (de, dt) in dq.iter() {
        if dt
            .translation
            .truncate()
            .distance(pt.translation.truncate())
            < ENEMY_SIZE.x / 2.0 + PLAYER_SIZE.x / 2.0
        {
            cmd.entity(de).despawn();
            events.write(PlayerHitMsg);
            break;
        }
    }
}

fn check_wave_complete(
    eq: Query<Entity, With<Enemy>>,
    aq: Query<&WaveAnnouncement>,
    mut events: MessageWriter<WaveCompleteMsg>,
) {
    if eq.is_empty() && aq.is_empty() {
        events.write(WaveCompleteMsg);
    }
}

// Background
fn starfield(
    time: Res<Time>,
    mut q: Query<(&mut Transform, &Star)>,
    wq: Query<&Window, With<PrimaryWindow>>,
) {
    let hh = wq.single().unwrap().height() / 2.0;
    for (mut t, s) in q.iter_mut() {
        t.translation.y -= s.speed * time.delta_secs();
        if t.translation.y < -hh {
            t.translation.y = hh;
        }
    }
}

// UI
fn update_ui(gd: Res<GameData>, mut q: Query<(&mut Text, &UiMarker)>) {
    for (mut t, m) in q.iter_mut() {
        t.0 = match m {
            UiMarker::Score => format!("SCORE: {}", gd.score),
            UiMarker::High => format!("HIGH: {}", gd.high_score),
            UiMarker::Lives => format!("LIVES: {}", gd.lives),
            UiMarker::Wave => format!("WAVE {}", gd.wave),
            _ => continue,
        };
    }
}

// Input
#[allow(clippy::too_many_arguments)]
fn game_input(
    mut cmd: Commands,
    kb: Res<ButtonInput<KeyCode>>,
    mut gd: ResMut<GameData>,
    mut ns: ResMut<NextState<GameState>>,
    state: Res<State<GameState>>,
    eq: Query<Entity, With<Enemy>>,
    bq: Query<Entity, With<Bullet>>,
    pq: Query<Entity, With<Player>>,
    aq: Query<Entity, With<WaveAnnouncement>>,
    wq: Query<&Window, With<PrimaryWindow>>,
    mut playback: Query<&mut Ym2149Playback>,
) {
    if kb.just_pressed(KeyCode::KeyR) {
        for e in eq.iter().chain(bq.iter()).chain(pq.iter()).chain(aq.iter()) {
            cmd.entity(e).despawn();
        }
        gd.reset();
        spawn_player(&mut cmd, wq.single().unwrap().height() / 2.0);
        spawn_wave_announcement(&mut cmd, 1);
        if let Ok(mut p) = playback.single_mut() {
            p.set_subsong(2);
            p.play();
        }
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
