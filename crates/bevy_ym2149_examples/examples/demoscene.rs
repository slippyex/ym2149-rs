//! Cube Faces Raymarch (ShaderToy Buffer A Port, single pass)
//! Mit Text-Overlay + YM2149 Playback.
//!
//! Shader: `shaders/cube_faces_singlepass.wgsl` (relative to assets directory)

use std::collections::HashMap;

use bevy::asset::AssetPlugin;
use bevy::asset::RenderAssetUsages;
use bevy::log::debug;
use bevy::math::primitives::Rectangle;
use bevy::{
    prelude::*,
    render::render_resource::{AsBindGroup, Extent3d, ShaderType, TextureDimension, TextureFormat},
    shader::{Shader, ShaderRef},
    sprite_render::{Material2d, Material2dPlugin, MeshMaterial2d},
    ui::widget::{ImageNode, NodeImageMode},
};
use bevy_mesh::Mesh2d;
use bevy_ym2149::{Ym2149AudioSource, Ym2149Playback, Ym2149Plugin};
use bevy_ym2149_examples::ASSET_BASE;

// === Easing Functions (Demoscene Style) =====================================
fn ease_out_quad(t: f32) -> f32 {
    1.0 - (1.0 - t) * (1.0 - t)
}

fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t) * (1.0 - t) * (1.0 - t)
}

fn ease_out_back_soft(t: f32) -> f32 {
    // Smooth ease-out-back with reduced overshoot for gentle bounce effect
    let c1 = 0.8; // Gentle overshoot amplitude
    let c3 = c1 + 1.0;
    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}

fn ease_out_bounce(t: f32) -> f32 {
    let n1 = 7.5625;
    let d1 = 2.75;

    if t < 1.0 / d1 {
        n1 * t * t
    } else if t < 2.0 / d1 {
        let t = t - 1.5 / d1;
        n1 * t * t + 0.75
    } else if t < 2.5 / d1 {
        let t = t - 2.25 / d1;
        n1 * t * t + 0.9375
    } else {
        let t = t - 2.625 / d1;
        n1 * t * t + 0.984375
    }
}

fn ease_out_elastic(t: f32) -> f32 {
    let c5 = (2.0 * std::f32::consts::PI) / 4.5;
    if t == 0.0 {
        0.0
    } else if t == 1.0 {
        1.0
    } else {
        (2.0_f32).powf(-10.0 * t) * ((t - 0.075) * c5).sin() + 1.0
    }
}

// === Animation Configuration Constants ========================================
// Swing Animation: Creates elliptical motion using sine/cosine 90° apart
// - Horizontal uses sin(t), Vertical uses cos(t) = sin(t + π/2)
// - Text and background swing with phase offset for visual separation
//
// Text Scaling Architecture:
// Final text size = base_size * zoom * animation_scale * BASE_TEXT_SCALE * viewport_scale
// Where:
//   - base_size: Bitmap font glyph dimensions
//   - zoom: Fade-out effect (0.0-1.0)
//   - animation_scale: Per-animation factor (BounceIn 0.05-1.0, ZoomIn 0.1-1.0, etc.)
//   - BASE_TEXT_SCALE: Global 1.5x multiplier (amplifies all animations)
//   - viewport_scale: Responsive scaling based on window width (0.8-1.5x)
//
// Example scaling chains:
//   - Typewriter at 1280px: 100px × 1.0 × 1.0 × 1.5 × 1.0 = 150px
//   - ZoomIn at 1280px: 100px × 1.0 × 1.0 × 1.5 × 1.0 = 150px
//   - Text at 800px width: 100px × 1.0 × 1.0 × 1.5 × 0.875 = 131px
const BASE_TEXT_SCALE: f32 = 1.5; // Base scale for all text (1.5x larger)
const DESIGN_WIDTH: f32 = 1280.0; // Reference viewport width for scaling calculations
const MIN_VIEWPORT_SCALE: f32 = 0.8; // Prevent text from becoming too small on narrow screens
const MAX_VIEWPORT_SCALE: f32 = 1.5; // Prevent text from becoming too large on wide screens
const SWING_FREQUENCY: f32 = 2.0; // Hz (0.5s period for full ellipse)
const SWING_AMPLITUDE_PX: f32 = 25.0; // Horizontal swing at 1280px width (~2%)
const SWING_VERTICAL_AMPLITUDE_PX: f32 = 40.0; // Vertical swing (stronger, ~3%)
const SWING_PHASE_OFFSET: f32 = 0.5; // ~28.6° phase offset between text and block
const BACKGROUND_OVERHANG_PX: f32 = 64.0; // Extra width on each side for swinging background
const BREATH_FREQUENCY: f32 = 1.6; // Hz
const BREATH_AMPLITUDE: f32 = 0.1; // Oscillation magnitude
const PULSE_SCALING: f32 = 0.6; // Scale factor for beat pulse energy
const STARTUP_FADE_DURATION: f32 = 2.5; // Black overlay fade-out duration (music starts at fade begin)

// Animation Timing Constants
const SIMPLE_FADE_DURATION: f32 = 1.2; // SimpleFade: total duration for alpha fade
const ELASTIC_REVEAL_TIME_PER_CHAR: f32 = 0.04; // ElasticReveal: 40ms per character
const BOUNCE_DURATION: f32 = 1.3; // BounceIn: dramatic bounce needs time for multiple bounces
const STAGGERED_CHAR_DELAY: f32 = 0.05; // StaggeredSlide: 50ms delay between each character
const STAGGERED_BASE_DURATION: f32 = 0.5; // StaggeredSlide: base duration after all chars revealed
const SLIDE_DISTANCE_PX: f32 = 100.0; // StaggeredSlide: horizontal slide distance
const GLOW_DURATION: f32 = 1.2; // GlowPulse: time for glow to fade
const GLOW_SCALE_DELTA: f32 = 0.2; // GlowPulse: how much larger text starts (1.2 → 1.0)
const GLOW_PULSE_FREQUENCY: f32 = 10.0; // GlowPulse: pulse frequency in Hz
const GLOW_PULSE_AMPLITUDE: f32 = 0.1; // GlowPulse: pulse amplitude
const BACKFLIP_DURATION: f32 = 1.0; // BackFlip: time for overshoot and settle
const BACKFLIP_MAX_SCALE: f32 = 1.1; // BackFlip: maximum scale during overshoot
const ZOOMIN_DURATION: f32 = 1.4; // ZoomIn: dramatic zoom with alpha fade
const ZOOMIN_START_SCALE: f32 = 0.1; // ZoomIn: starting scale
const ZOOMIN_SCALE_RANGE: f32 = 0.9; // ZoomIn: scale range (0.1 to 1.0)

// === Material + Uniforms =====================================================

#[derive(AsBindGroup, TypePath, Debug, Clone, Asset, Default)]
pub struct CubeFacesMaterial {
    #[uniform(0)]
    params: CubeParams,
}
impl Material2d for CubeFacesMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path("shaders/cube_faces_singlepass.wgsl".into())
    }
}

#[derive(ShaderType, Clone, Copy, Debug)]
struct CubeParams {
    time: f32,
    width: f32,
    height: f32,
    mouse: Vec4, // optional belegt, aktuell 0
    frame: u32,
    crt_enabled: u32,
}
impl Default for CubeParams {
    fn default() -> Self {
        Self {
            time: 0.0,
            width: 1280.0,
            height: 720.0,
            mouse: Vec4::ZERO,
            frame: 0,
            crt_enabled: 1,
        }
    }
}

#[derive(Resource)]
struct MaterialHandle(Handle<CubeFacesMaterial>);

#[derive(Resource)]
struct StartupFade {
    shader: Handle<Shader>,
    state: StartupFadePhase,
    timer: f32,
    duration: f32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StartupFadePhase {
    Loading,
    Fading,
    Done,
}

#[derive(Component)]
struct StartupFadeOverlay;

#[derive(Resource)]
struct PendingSurface {
    mesh: Handle<Mesh>,
    scale: Vec3,
    spawned: bool,
}

#[derive(Resource)]
struct CrtState {
    enabled: bool,
}

// === Overlay + Text Writer ===================================================

const BITMAP_CELL_SIZE: UVec2 = UVec2::new(16, 16);
const BITMAP_LETTER_SPACING: u32 = 2;
const BITMAP_FONT_LAYOUT: [&str; 3] = [
    " !\"#$%&'()*+,-./0123",
    "456789:;<=>?@ABCDEFG",
    "HIJKLMNOPQRSTUVWXYZ[",
];

const PLAY_MUSIC: bool = true;
const YM_TRACK_PATH: &str = "music/ND-Toxygene.ym";

#[derive(Resource)]
struct BitmapFont {
    image: Handle<Image>,
    glyph_map: HashMap<char, UVec2>,
    cell_size: UVec2,
    letter_spacing: u32,
    default_coord: UVec2,
}

impl BitmapFont {
    fn new(image: Handle<Image>) -> Self {
        let mut glyph_map = HashMap::new();
        for (row, line) in BITMAP_FONT_LAYOUT.iter().enumerate() {
            for (col, ch) in line.chars().enumerate() {
                glyph_map.insert(ch, UVec2::new(col as u32, row as u32));
            }
        }
        let default_coord = glyph_map.get(&' ').copied().unwrap_or(UVec2::ZERO);
        Self {
            image,
            glyph_map,
            cell_size: BITMAP_CELL_SIZE,
            letter_spacing: BITMAP_LETTER_SPACING,
            default_coord,
        }
    }

    fn coord_for(&self, ch: char) -> UVec2 {
        self.glyph_map
            .get(&ch)
            .copied()
            .unwrap_or(self.default_coord)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimationType {
    Typewriter,     // Original: character by character reveal
    BounceIn,       // Whole text scales from small with bounce (easeOutBounce)
    StaggeredSlide, // Characters slide in from left with staggered timing
    SimpleFade,     // Whole text fades in with alpha (easeOutCubic)
    ElasticReveal,  // Character-by-character reveal with elastic easing
    GlowPulse,      // Pulsing glow effect with scale (easeOutElastic)
    ScaleDown,      // Text scales down during fade-out (easeOutQuad)
    BackFlip,       // Text flips in with overshoot (easeOutBack)
    ZoomIn,         // Text zooms in from small to large (easeOutElastic)
}
impl Default for AnimationType {
    fn default() -> Self {
        Self::Typewriter
    }
}

#[derive(Clone, Message)]
pub struct PushOverlayText {
    pub text: String,
    pub cps: f32,
    pub dwell: f32,
    pub fade_out: f32,
    pub animation: AnimationType,
}
#[derive(Resource, Default)]
struct TextQueue(Vec<PushOverlayText>);

#[derive(Resource, Clone)]
struct OverlayScript {
    lines: Vec<PushOverlayText>,
}
impl Default for OverlayScript {
    fn default() -> Self {
        Self {
            lines: vec![
                PushOverlayText {
                    text: "RULING THE SCENE SINCE 1991".into(),
                    cps: 35.0,
                    dwell: 1.2,
                    fade_out: 0.6,
                    animation: AnimationType::Typewriter,
                },
                PushOverlayText {
                    text: "VECTRONIX PRESENTS".into(),
                    cps: 38.0,
                    dwell: 1.2,
                    fade_out: 0.6,
                    animation: AnimationType::SimpleFade,
                },
                PushOverlayText {
                    text: "AN OLDSKOOL DEMOSCENE INTRO".into(),
                    cps: 52.0,
                    dwell: 1.3,
                    fade_out: 0.6,
                    animation: AnimationType::BounceIn,
                },
                PushOverlayText {
                    text: "ENTIRELY WRITTEN IN RUST, BEVY AND WGSL".into(),
                    cps: 60.0,
                    dwell: 1.5,
                    fade_out: 0.75,
                    animation: AnimationType::BounceIn,
                },
                PushOverlayText {
                    text: "INCLUDING THE YM2149 EMULATOR CRATE".into(),
                    cps: 50.0,
                    dwell: 1.3,
                    fade_out: 0.6,
                    animation: AnimationType::ElasticReveal,
                },
                PushOverlayText {
                    text: "WITH ORIGINAL MUSIC FROM THE ATARI ST".into(),
                    cps: 40.0,
                    dwell: 1.1,
                    fade_out: 0.5,
                    animation: AnimationType::StaggeredSlide,
                },
                PushOverlayText {
                    text: "USING THE BEVY YM2149 PLUGIN".into(),
                    cps: 35.0,
                    dwell: 1.3,
                    fade_out: 0.6,
                    animation: AnimationType::BounceIn,
                },
                PushOverlayText {
                    text: "ENJOY THE SOUND OF THE 90S ERA".into(),
                    cps: 40.0,
                    dwell: 1.2,
                    fade_out: 0.5,
                    animation: AnimationType::BounceIn,
                },
                PushOverlayText {
                    text: "MUSIC BY TAO OF ACF".into(),
                    cps: 45.0,
                    dwell: 1.2,
                    fade_out: 0.55,
                    animation: AnimationType::Typewriter,
                },
                PushOverlayText {
                    text: "GREETINGS GO OUT TO".into(),
                    cps: 30.0,
                    dwell: 2.0,
                    fade_out: 1.0,
                    animation: AnimationType::BounceIn,
                },
                PushOverlayText {
                    text: "ALL ACTIVE AND RETIRED SCENERS".into(),
                    cps: 32.0,
                    dwell: 2.5,
                    fade_out: 1.2,
                    animation: AnimationType::Typewriter,
                },
                PushOverlayText {
                    text: "AND OF COURSE THE RUST COMMUNITY".into(),
                    cps: 28.0,
                    dwell: 2.0,
                    fade_out: 1.0,
                    animation: AnimationType::ElasticReveal,
                },
            ],
        }
    }
}

#[derive(Component)]
struct OverlayText;
#[derive(Component)]
struct OverlayBackground;
#[derive(Resource)]
struct TextWriterState {
    timer: f32,
    visible_chars: usize,
    phase: Phase,
    current: Option<PushOverlayText>,
    alpha: f32,
    animation_type: AnimationType,
    scale: f32,          // For BounceIn, GlowPulse, BackFlip, ZoomIn, ScaleDown
    x_offset: f32,       // For StaggeredSlide horizontal movement
    y_offset: f32,       // For vertical animations
    glow_intensity: f32, // For GlowPulse
    swing_h: f32,        // Horizontal swing offset (calculated by apply_swing_animation)
    swing_v: f32,        // Vertical swing offset (calculated by apply_swing_animation)
    bg_swing_h: f32,     // Background horizontal swing offset
    bg_swing_v: f32,     // Background vertical swing offset
    viewport_scale: f32, // Responsive scale based on window width (calculated by apply_swing_animation)
}
impl Default for TextWriterState {
    fn default() -> Self {
        Self {
            timer: 0.0,
            visible_chars: 0,
            phase: Phase::Idle,
            current: None,
            alpha: 0.0,
            animation_type: AnimationType::Typewriter,
            scale: 1.0,
            x_offset: 0.0,
            y_offset: 0.0,
            glow_intensity: 0.0,
            swing_h: 0.0,
            swing_v: 0.0,
            bg_swing_h: 0.0,
            bg_swing_v: 0.0,
            viewport_scale: 1.0, // Default to 1.0x at design width (1280px)
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    Idle,
    Typing,
    Dwell,
    FadeOut,
}

// === Beat-Pulse (Glow) ======================================================

#[derive(Resource, Default)]
struct BeatPulse {
    energy: f32,
}

// === App ====================================================================

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "VeCTRONiX - YM2149 Emulator".into(),
                        resolution: (1280, 720).into(),
                        present_mode: bevy::window::PresentMode::AutoVsync,
                        resizable: true,
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin {
                    file_path: ASSET_BASE.into(),
                    ..default()
                }),
        )
        .add_plugins((
            Material2dPlugin::<CubeFacesMaterial>::default(),
            Ym2149Plugin::default(),
        ))
        .add_message::<PushOverlayText>()
        .add_systems(
            Startup,
            (
                setup,
                setup_text_overlay,
                setup_startup_fade,
                init_resources,
            ),
        )
        .add_systems(
            Update,
            (
                spawn_surface_when_ready,
                update_startup_fade,
                toggle_crt,
                handle_push_events,
                feed_overlay_script,
                apply_swing_animation, // Calculate swing offsets (updates state)
                update_uniforms,
                typewriter_update, // Apply text animation + swing offsets to text layout
                apply_background_swing, // Apply background swing offsets
            ),
        )
        .run();
}

// === Setup ==================================================================

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    windows: Query<&Window>,
) {
    commands.spawn(Camera2d);

    if PLAY_MUSIC {
        let ym_handle: Handle<Ym2149AudioSource> = asset_server.load(YM_TRACK_PATH);
        let mut playback = Ym2149Playback::from_asset(ym_handle);
        playback.play();
        commands.spawn(playback);
    }

    // Fullscreen Quad (deferred spawn)
    let mesh = meshes.add(Mesh::from(Rectangle::new(2.0, 2.0)));

    let window_size = windows
        .iter()
        .next()
        .map(|w| Vec2::new(w.resolution.width(), w.resolution.height()))
        .unwrap_or(Vec2::new(1280.0, 720.0));

    let quad_scale = Vec3::new(window_size.x * 0.5, window_size.y * 0.5, 1.0);

    commands.insert_resource(PendingSurface {
        mesh,
        scale: quad_scale,
        spawned: false,
    });

    // Shader Hot Reload
    let shader_handle: Handle<Shader> = asset_server.load("shaders/cube_faces_singlepass.wgsl");
    commands.insert_resource(StartupFade {
        shader: shader_handle,
        state: StartupFadePhase::Loading,
        timer: 0.0,
        duration: STARTUP_FADE_DURATION,
    });
}

// === Text Overlay ===========================================================

fn setup_text_overlay(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
) {
    let font_image = asset_server.load("fonts/demoscene_font.png");
    commands.insert_resource(BitmapFont::new(font_image));

    let extent = Extent3d {
        width: 1,
        height: 1,
        depth_or_array_layers: 1,
    };
    let placeholder = Image::new_fill(
        extent,
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    let bitmap_image_handle = images.add(placeholder);

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            Visibility::Visible,
            InheritedVisibility::VISIBLE,
            GlobalZIndex(100),
            Name::new("OverlayTextRoot"),
        ))
        .with_children(|root| {
            // Background block - full width behind the text
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    height: Val::Px(120.0),
                    left: Val::Px(-BACKGROUND_OVERHANG_PX),
                    right: Val::Px(-BACKGROUND_OVERHANG_PX),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.9)),
                Visibility::Visible,
                InheritedVisibility::VISIBLE,
                GlobalZIndex(99),
                OverlayBackground,
                Name::new("TextBackground"),
            ));

            // Text overlay
            root.spawn((
                ImageNode::from(bitmap_image_handle)
                    .with_color(Color::srgba(0.95, 0.95, 1.0, 1.0))
                    .with_mode(NodeImageMode::Stretch),
                Node {
                    width: Val::Px(0.0),
                    height: Val::Px(0.0),
                    align_self: AlignSelf::Center,
                    justify_self: JustifySelf::Center,
                    ..default()
                },
                Visibility::Visible,
                InheritedVisibility::VISIBLE,
                GlobalZIndex(100),
                OverlayText,
                Name::new("MainBitmapText"),
            ));
        });
}

fn setup_startup_fade(mut commands: Commands) {
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(0.0),
            bottom: Val::Px(0.0),
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 1.0)),
        Visibility::Visible,
        InheritedVisibility::VISIBLE,
        GlobalZIndex(20_000),
        StartupFadeOverlay,
        Name::new("StartupFadeOverlay"),
    ));
}

fn init_resources(mut commands: Commands) {
    commands.insert_resource(TextQueue::default());
    commands.insert_resource(OverlayScript::default());
    commands.insert_resource(TextWriterState::default());
    commands.insert_resource(BeatPulse { energy: 0.0 });
    commands.insert_resource(CrtState { enabled: true });
}

fn spawn_surface_when_ready(
    asset_server: Res<AssetServer>,
    pending: Option<ResMut<PendingSurface>>,
    mut materials: ResMut<Assets<CubeFacesMaterial>>,
    mut commands: Commands,
    fade: Res<StartupFade>,
) {
    let Some(mut pending) = pending else {
        return;
    };

    if pending.spawned {
        return;
    }

    if !asset_server.is_loaded_with_dependencies(&fade.shader) {
        return;
    }

    let material_handle = materials.add(CubeFacesMaterial::default());
    commands.spawn((
        Mesh2d(pending.mesh.clone()),
        MeshMaterial2d(material_handle.clone()),
        Transform::from_scale(pending.scale),
        GlobalTransform::default(),
        Visibility::default(),
        Name::new("CubeFacesSurface"),
    ));
    commands.insert_resource(MaterialHandle(material_handle));
    pending.spawned = true;
}

fn toggle_crt(keys: Res<ButtonInput<KeyCode>>, mut crt: ResMut<CrtState>) {
    if keys.just_pressed(KeyCode::KeyC) {
        crt.enabled = !crt.enabled;
    }
}

// === Uniform Update =========================================================

fn update_uniforms(
    time: Res<Time>,
    windows: Query<&Window>,
    mut materials: ResMut<Assets<CubeFacesMaterial>>,
    mat: Option<Res<MaterialHandle>>,
    crt: Option<Res<CrtState>>,
) {
    let Some(mat) = mat else {
        return;
    };
    let Some(window) = windows.iter().next() else {
        return;
    };
    let Some(material) = materials.get_mut(&mat.0) else {
        return;
    };

    material.params.time = time.elapsed_secs();
    material.params.width = window.resolution.width();
    material.params.height = window.resolution.height();
    material.params.frame = material.params.frame.wrapping_add(1);
    if let Some(crt_state) = crt {
        material.params.crt_enabled = if crt_state.enabled { 1 } else { 0 };
    }
}

fn update_startup_fade(
    time: Res<Time>,
    asset_server: Res<AssetServer>,
    mut fade: ResMut<StartupFade>,
    mut overlay: Query<(&mut BackgroundColor, &mut Visibility), With<StartupFadeOverlay>>,
) {
    let Ok((mut bg, mut visibility)) = overlay.single_mut() else {
        return;
    };

    match fade.state {
        StartupFadePhase::Loading => {
            bg.0 = bg.0.with_alpha(1.0);
            *visibility = Visibility::Visible;
            if asset_server.is_loaded_with_dependencies(&fade.shader) {
                fade.state = StartupFadePhase::Fading;
                fade.timer = 0.0;
            }
        }
        StartupFadePhase::Fading => {
            fade.timer += time.delta_secs();
            let progress = (fade.timer / fade.duration).clamp(0.0, 1.0);
            let eased = ease_out_cubic(progress);
            bg.0 = bg.0.with_alpha(1.0 - eased);
            if progress >= 1.0 {
                fade.state = StartupFadePhase::Done;
                bg.0 = bg.0.with_alpha(0.0);
                *visibility = Visibility::Hidden;
            }
        }
        StartupFadePhase::Done => {
            bg.0 = bg.0.with_alpha(0.0);
            *visibility = Visibility::Hidden;
        }
    }
}

// === Overlay Writer ============================================

fn handle_push_events(mut evr: MessageReader<PushOverlayText>, mut queue: ResMut<TextQueue>) {
    for ev in evr.read() {
        queue.0.push(ev.clone());
    }
}
fn feed_overlay_script(
    script: Res<OverlayScript>,
    state: Res<TextWriterState>,
    mut queue: ResMut<TextQueue>,
) {
    if state.phase == Phase::Idle && queue.0.is_empty() && !script.lines.is_empty() {
        queue.0.extend(script.lines.iter().cloned());
    }
}

fn typewriter_update(
    time: Res<Time>,
    mut queue: ResMut<TextQueue>,
    mut state: ResMut<TextWriterState>,
    mut main_query: Query<(&mut ImageNode, &mut Node), With<OverlayText>>,
    pulse: Res<BeatPulse>,
    font: Option<Res<BitmapFont>>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(font) = font else {
        return;
    };
    if images.get(&font.image).is_none() {
        return;
    }

    let dt = time.delta_secs();

    if state.phase == Phase::Idle {
        start_next_message(&mut state, &mut queue);
    }

    if let Some(current) = state.current.clone() {
        let pulse_strength = (pulse.energy * PULSE_SCALING).clamp(0.0, 1.2);
        let breath = (time.elapsed_secs() * BREATH_FREQUENCY).sin() * BREATH_AMPLITUDE + 0.9;

        match state.phase {
            Phase::Typing => {
                state.timer += dt;
                let total = current.text.chars().count();
                let typing_duration = total as f32 / current.cps;

                match state.animation_type {
                    AnimationType::Typewriter => {
                        // Original: character by character
                        state.visible_chars = (state.timer * current.cps).floor() as usize;
                        state.alpha = 1.0;
                    }
                    AnimationType::BounceIn => {
                        // All text at once with DRAMATIC bounce (multiple bounces)
                        state.visible_chars = total;
                        let t = (state.timer / BOUNCE_DURATION).clamp(0.0, 1.0);
                        let bounce_t = ease_out_bounce(t);
                        state.scale = 0.05 + bounce_t * 0.95; // Scales from 0.05 to 1.0 - massive bounce!
                        state.alpha = 1.0;
                    }
                    AnimationType::StaggeredSlide => {
                        // Characters slide in from the side with staggered timing
                        let total_duration =
                            total as f32 * STAGGERED_CHAR_DELAY + STAGGERED_BASE_DURATION;
                        let t = (state.timer / total_duration).clamp(0.0, 1.0);

                        // Calculate how many characters should be visible
                        state.visible_chars =
                            ((t * total_duration) / STAGGERED_CHAR_DELAY) as usize;
                        state.visible_chars = state.visible_chars.min(total);

                        // Horizontal slide effect for all visible characters
                        let slide_t = ease_out_cubic(t);
                        state.x_offset = -SLIDE_DISTANCE_PX * (1.0 - slide_t); // Slide from -100px to 0
                        state.alpha = 1.0;
                    }
                    AnimationType::SimpleFade => {
                        // Whole text fades in with alpha
                        state.visible_chars = total;
                        let t = (state.timer / SIMPLE_FADE_DURATION).clamp(0.0, 1.0);
                        state.alpha = ease_out_cubic(t);
                    }
                    AnimationType::ElasticReveal => {
                        // Characters revealed with elastic easing (typewriter variant)
                        let t = (state.timer / (total as f32 * ELASTIC_REVEAL_TIME_PER_CHAR))
                            .clamp(0.0, 1.0);
                        state.visible_chars = (ease_out_elastic(t) * total as f32) as usize;
                        state.visible_chars = state.visible_chars.min(total);
                        state.alpha = 1.0;
                    }
                    AnimationType::GlowPulse => {
                        // Text pulses in with a glow-like effect
                        state.visible_chars = total;
                        let t = (state.timer / GLOW_DURATION).clamp(0.0, 1.0);

                        // Create pulsing alpha effect
                        let pulse = (state.timer * GLOW_PULSE_FREQUENCY).sin()
                            * GLOW_PULSE_AMPLITUDE
                            + (1.0 - GLOW_PULSE_AMPLITUDE);
                        state.alpha = ease_out_cubic(t) * pulse;

                        // Scale pulses slightly for glow effect
                        state.scale = 1.0 + (1.0 - t) * GLOW_SCALE_DELTA; // Start at 1.2, shrink to 1.0
                        state.glow_intensity = (1.0 - t) * 2.0;
                    }
                    AnimationType::ScaleDown => {
                        // Text appears normally during typing
                        state.visible_chars = total;
                        state.scale = 1.0;
                        state.alpha = 1.0;
                    }
                    AnimationType::BackFlip => {
                        // Text flips in with overshoot
                        state.visible_chars = total;
                        let t = (state.timer / BACKFLIP_DURATION).clamp(0.0, 1.0);
                        let ease_t = ease_out_back_soft(t);
                        state.scale = ease_t * BACKFLIP_MAX_SCALE; // 0 to 1.1 with overshoot
                        state.alpha = 1.0;
                    }
                    AnimationType::ZoomIn => {
                        // Text zooms in from small to large
                        state.visible_chars = total;
                        let t = (state.timer / ZOOMIN_DURATION).clamp(0.0, 1.0);
                        let ease_t = ease_out_elastic(t);
                        state.scale = ZOOMIN_START_SCALE + ease_t * ZOOMIN_SCALE_RANGE; // Scales from 0.1 to 1.0
                        state.alpha = t;
                    }
                }

                // Check if animation is complete
                if state.timer
                    >= match state.animation_type {
                        AnimationType::Typewriter => typing_duration,
                        AnimationType::BounceIn => BOUNCE_DURATION,
                        AnimationType::StaggeredSlide => {
                            total as f32 * STAGGERED_CHAR_DELAY + STAGGERED_BASE_DURATION
                        }
                        AnimationType::SimpleFade => SIMPLE_FADE_DURATION,
                        AnimationType::ElasticReveal => total as f32 * ELASTIC_REVEAL_TIME_PER_CHAR,
                        AnimationType::GlowPulse => GLOW_DURATION,
                        AnimationType::ScaleDown => typing_duration,
                        AnimationType::BackFlip => BACKFLIP_DURATION,
                        AnimationType::ZoomIn => ZOOMIN_DURATION,
                    }
                {
                    state.visible_chars = total;
                    state.phase = Phase::Dwell;
                    state.timer = 0.0;
                    state.scale = 1.0;
                    state.x_offset = 0.0;
                    state.y_offset = 0.0;
                    state.glow_intensity = 0.0;
                }
            }
            Phase::Dwell => {
                state.timer += dt;
                if state.timer >= current.dwell {
                    state.phase = Phase::FadeOut;
                    state.timer = 0.0;
                }
                state.alpha = 1.0;
            }
            Phase::FadeOut => {
                state.timer += dt;
                let t = (state.timer / current.fade_out).clamp(0.0, 1.0);

                // Apply fade-out animation based on type
                match state.animation_type {
                    AnimationType::ScaleDown => {
                        // Scale down while fading out
                        let ease_t = ease_out_quad(t);
                        state.scale = 1.0 - ease_t * 0.7; // Scale from 1.0 to 0.3
                        state.alpha = 1.0 - ease_t; // Fade from 1.0 to 0.0
                    }
                    _ => {
                        // Default: just fade alpha
                        state.alpha = 1.0 - t;
                    }
                }

                if t >= 1.0 {
                    if !queue.0.is_empty() {
                        queue.0.remove(0);
                    }
                    if let Ok((mut image, mut layout)) = main_query.single_mut() {
                        rebuild_bitmap_text("", &font, &image.image, &mut images);
                        layout.width = Val::Px(0.0);
                        layout.height = Val::Px(0.0);
                        layout.min_width = Val::Px(0.0);
                        layout.min_height = Val::Px(0.0);
                        layout.max_width = Val::Px(0.0);
                        layout.max_height = Val::Px(0.0);
                        image.color.set_alpha(0.0);
                    }
                    state.phase = Phase::Idle;
                    state.current = None;
                    state.alpha = 0.0;
                    return;
                }
            }
            Phase::Idle => {}
        }

        let visible: String = current.text.chars().take(state.visible_chars).collect();
        if let Ok((mut image, mut layout)) = main_query.single_mut() {
            let size = rebuild_bitmap_text(&visible, &font, &image.image, &mut images);
            let zoom = if matches!(state.phase, Phase::FadeOut) {
                state.alpha.clamp(0.0, 1.0)
            } else {
                1.0
            };

            // Apply scale for animation types that use it
            let scale_factor = match state.animation_type {
                AnimationType::BounceIn
                | AnimationType::GlowPulse
                | AnimationType::ScaleDown
                | AnimationType::BackFlip
                | AnimationType::ZoomIn => state.scale,
                _ => 1.0,
            };

            // Final text dimensions: base * zoom * animation_scale * base_scale * viewport_scale
            let width =
                (size.x as f32 * zoom * scale_factor * BASE_TEXT_SCALE * state.viewport_scale)
                    .max(0.0);
            let height =
                (size.y as f32 * zoom * scale_factor * BASE_TEXT_SCALE * state.viewport_scale)
                    .max(0.0);
            layout.width = Val::Px(width);
            layout.height = Val::Px(height);
            layout.min_width = Val::Px(width);
            layout.min_height = Val::Px(height);
            layout.max_width = Val::Px(width);
            layout.max_height = Val::Px(height);

            // Apply horizontal offsets (swing + animation-specific)
            let base_left = match state.animation_type {
                AnimationType::StaggeredSlide => state.x_offset,
                _ => 0.0,
            };
            layout.margin.left = Val::Px(base_left + state.swing_h);

            // Apply vertical offsets (swing + animation-specific)
            layout.margin.top = Val::Px(state.y_offset + state.swing_v);

            if let Some(img) = images.get(&image.image) {
                debug!(target: "overlay", "text='{}' size=({}, {}) img_size={}x{} data_len={}",
                    visible, width, height,
                    img.texture_descriptor.size.width,
                    img.texture_descriptor.size.height,
                    img.data.as_ref().map(|d| d.len()).unwrap_or(0)
                );
            } else {
                debug!(target: "overlay", "text='{}' size=({}, {}) IMAGE NOT FOUND", visible, width, 16.0);
            }

            let brightness = 0.9 + 0.1 * pulse_strength;
            let cool_shift = 0.02 * (breath - 0.9);

            // Apply glow for GlowPulse animation
            let (r, g, b) = if state.animation_type == AnimationType::GlowPulse {
                let glow = state.glow_intensity;
                (
                    (brightness + cool_shift + glow * 2.0).clamp(0.0, 1.0),
                    (brightness + cool_shift * 0.5 + glow).clamp(0.0, 1.0),
                    (brightness + glow * 0.5).clamp(0.0, 1.0),
                )
            } else {
                (
                    (brightness + cool_shift).clamp(0.0, 1.0),
                    (brightness + cool_shift * 0.5).clamp(0.0, 1.0),
                    1.0,
                )
            };

            image.color = Color::srgba(r, g, b, state.alpha);
        }
    }
}

fn start_next_message(state: &mut TextWriterState, queue: &mut TextQueue) {
    if let Some(msg) = queue.0.first().cloned() {
        state.animation_type = msg.animation;
        state.current = Some(msg);
        state.timer = 0.0;
        state.visible_chars = 0;
        state.phase = Phase::Typing;
        state.alpha = 1.0;

        // Initialize animation-specific values
        match state.animation_type {
            AnimationType::BounceIn => {
                state.scale = 0.1;
                state.alpha = 1.0;
            }
            AnimationType::StaggeredSlide => {
                state.x_offset = -SLIDE_DISTANCE_PX; // Start off-screen to the left
                state.y_offset = 0.0;
                state.alpha = 1.0;
            }
            AnimationType::SimpleFade => {
                state.alpha = 0.0; // Start transparent for fade-in
            }
            AnimationType::ElasticReveal => {
                state.alpha = 1.0;
            }
            AnimationType::GlowPulse => {
                state.scale = 1.0 + GLOW_SCALE_DELTA; // Start slightly larger
                state.glow_intensity = 2.0;
                state.alpha = 0.0;
            }
            AnimationType::ScaleDown => {
                state.scale = 1.0; // Normal size during typing
                state.alpha = 1.0; // Fully visible
            }
            AnimationType::BackFlip => {
                state.scale = 0.0;
                state.alpha = 1.0;
            }
            AnimationType::ZoomIn => {
                state.scale = ZOOMIN_START_SCALE; // Start small
                state.alpha = 0.0; // Start transparent
            }
            _ => {}
        }
    } else {
        state.current = None;
        state.phase = Phase::Idle;
        state.alpha = 0.0;
    }
}

fn apply_swing_animation(
    time: Res<Time>,
    mut state: ResMut<TextWriterState>,
    windows: Query<&Window>,
) {
    let elapsed = time.elapsed_secs();

    // Get actual window width and calculate responsive scales
    let window_width = windows
        .iter()
        .next()
        .map(|w| w.resolution.width())
        .unwrap_or(DESIGN_WIDTH);

    // Calculate swing amplitudes responsive to window width
    let swing_h_amplitude = window_width * (SWING_AMPLITUDE_PX / DESIGN_WIDTH);
    let swing_v_amplitude = window_width * (SWING_VERTICAL_AMPLITUDE_PX / DESIGN_WIDTH);

    // Calculate viewport scale for responsive text sizing
    // Scales between 0.8x and 1.5x based on window width relative to design width
    state.viewport_scale =
        (window_width / DESIGN_WIDTH).clamp(MIN_VIEWPORT_SCALE, MAX_VIEWPORT_SCALE);

    // Text: base phase - creates elliptical motion using sin/cos 90° apart
    let text_phase = elapsed * SWING_FREQUENCY;
    state.swing_h = text_phase.sin() * swing_h_amplitude;
    state.swing_v = text_phase.cos() * swing_v_amplitude;

    // Background block: offset phase for visually distinct motion
    let bg_phase = text_phase + SWING_PHASE_OFFSET;
    state.bg_swing_h = bg_phase.sin() * swing_h_amplitude;
    state.bg_swing_v = bg_phase.cos() * swing_v_amplitude;
}

fn apply_background_swing(
    state: Res<TextWriterState>,
    mut bg_query: Query<&mut Node, With<OverlayBackground>>,
) {
    for mut layout in bg_query.iter_mut() {
        layout.margin.left = Val::Px(state.bg_swing_h);
        layout.margin.top = Val::Px(state.bg_swing_v);
    }
}

fn rebuild_bitmap_text(
    text: &str,
    font: &BitmapFont,
    handle: &Handle<Image>,
    images: &mut Assets<Image>,
) -> UVec2 {
    if let Some(font_image) = images.get_mut(&font.image) {
        if font_image.texture_descriptor.format != TextureFormat::Rgba8UnormSrgb {
            if let Some(converted) = font_image.convert(TextureFormat::Rgba8UnormSrgb) {
                *font_image = converted;
            }
        }
    }
    let Some(font_image) = images.get(&font.image) else {
        return UVec2::ZERO;
    };
    let Some(font_pixels) = font_image.data.clone() else {
        return UVec2::ZERO;
    };
    if font_pixels.is_empty() {
        return UVec2::ZERO;
    }

    let texture_width = font_image.texture_descriptor.size.width;
    let texture_height = font_image.texture_descriptor.size.height;

    let Some(image) = images.get_mut(handle) else {
        return UVec2::ZERO;
    };

    if text.is_empty() {
        image.resize(Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        });
        image.data = Some(vec![0, 0, 0, 0]);
        return UVec2::new(1, 1);
    }

    let glyphs: Vec<char> = text.chars().collect();
    let cell_w = font.cell_size.x;
    let cell_h = font.cell_size.y;
    let spacing = font.letter_spacing;
    let width = glyphs.len() as u32 * (cell_w + spacing) - spacing;
    let height = cell_h;
    let mut output = vec![0u8; (width * height * 4) as usize];
    let row_span = texture_width as usize;
    let tiles_x = texture_width / cell_w;
    let tiles_y = texture_height / cell_h;
    let pixel_stride =
        (font_pixels.len() / (texture_width as usize * texture_height as usize)).max(1);

    for (index, ch) in glyphs.iter().enumerate() {
        let mut coord = font.coord_for(*ch);
        if coord.x >= tiles_x || coord.y >= tiles_y {
            coord = font.default_coord;
        }
        let src_x = (coord.x * cell_w) as usize;
        let src_y = (coord.y * cell_h) as usize;
        let dest_x = (index as u32) * (cell_w + spacing);
        for y in 0..cell_h as usize {
            let src_row = src_y + y;
            for x in 0..cell_w as usize {
                let src_col = src_x + x;
                let src_index = (src_row * row_span + src_col) * pixel_stride;
                let dst_index = (((y as u32) * width + dest_x + x as u32) * 4) as usize;
                let (r, g, b, _a) = match pixel_stride {
                    4 => (
                        font_pixels[src_index],
                        font_pixels[src_index + 1],
                        font_pixels[src_index + 2],
                        font_pixels[src_index + 3],
                    ),
                    3 => {
                        let r = font_pixels[src_index];
                        let g = font_pixels[src_index + 1];
                        let b = font_pixels[src_index + 2];
                        let alpha = r.max(g).max(b);
                        (r, g, b, alpha)
                    }
                    2 => {
                        let l = font_pixels[src_index];
                        let a = font_pixels[src_index + 1];
                        (l, l, l, a)
                    }
                    _ => {
                        let l = font_pixels[src_index];
                        (l, l, l, l)
                    }
                };
                // Use black as transparent (chroma key)
                let luminance = (r as u32 + g as u32 + b as u32) / 3;
                let alpha = if luminance < 64 { 0 } else { 255 };

                output[dst_index] = r;
                output[dst_index + 1] = g;
                output[dst_index + 2] = b;
                output[dst_index + 3] = alpha;
            }
        }
    }

    if image.texture_descriptor.size.width != width
        || image.texture_descriptor.size.height != height
    {
        image.resize(Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        });
    }
    image.data = Some(output);
    #[cfg(debug_assertions)]
    if let Some(sample) = images
        .get(handle)
        .and_then(|img| img.data.as_ref())
        .map(|d| d.iter().take(8).copied().collect::<Vec<_>>())
    {
        debug!(target: "overlay", "sample_pixels={:?}", sample);
    }
    UVec2::new(width, height)
}
