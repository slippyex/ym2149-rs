//! Cube Faces Raymarch (ShaderToy Buffer A Port, single pass)
//! Mit Text-Overlay + YM2149 Playback.
//!
//! Shader: `shaders/oldschool.wgsl` (relative to assets directory)

use std::{collections::HashMap, fs};

use bevy::{
    app::AppExit,
    asset::{AssetPlugin, RenderAssetUsages},
    camera::{ClearColorConfig, RenderTarget, visibility::RenderLayers},
    log::debug,
    math::primitives::Rectangle,
    prelude::*,
    render::render_resource::{
        AsBindGroup, Extent3d, ShaderType, TextureDescriptor, TextureDimension, TextureFormat,
        TextureUsages,
    },
    shader::{Shader, ShaderRef},
    sprite_render::{AlphaMode2d, Material2d, Material2dPlugin, MeshMaterial2d},
    ui::{
        IsDefaultUiCamera,
        widget::{ImageNode, NodeImageMode},
    },
    window::{MonitorSelection, PrimaryWindow, WindowMode, WindowResized},
};
use bevy_mesh::Mesh2d;
use bevy_ym2149::{Ym2149AudioSource, Ym2149Playback, Ym2149Plugin, Ym2149Settings};
use bevy_ym2149_examples::ASSET_BASE;
use bevy_ym2149_viz::{SpectrumBar, Ym2149VizPlugin};

// === Easing Functions (Demoscene Style) =====================================
fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t) * (1.0 - t) * (1.0 - t)
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
// Text Layout Configuration
struct TextLayoutConfig;
impl TextLayoutConfig {
    const BASE_SCALE: f32 = 1.5;
    const DESIGN_WIDTH: f32 = 1280.0;
    const MIN_VIEWPORT_SCALE: f32 = 0.8;
    const MAX_VIEWPORT_SCALE: f32 = 1.5;
    const BACKGROUND_OVERHANG_PX: f32 = 64.0;
}

// Background Swing Animation
struct SwingConfig;
impl SwingConfig {
    const FREQUENCY: f32 = 2.0;
    const AMPLITUDE_H_PX: f32 = 25.0;
    const AMPLITUDE_V_PX: f32 = 40.0;
    const PHASE_OFFSET: f32 = 0.5;
}

// Visual Effects
struct VisualEffectsConfig;
impl VisualEffectsConfig {
    const BREATH_FREQUENCY: f32 = 1.6;
    const BREATH_AMPLITUDE: f32 = 0.1;
    const PULSE_SCALING: f32 = 0.6;
    const STARTUP_FADE_DURATION: f32 = 2.5;
}

// Simple Fade Animation
struct SimpleFadeConfig;
impl SimpleFadeConfig {
    const DURATION: f32 = 1.2;
}

// Elastic Reveal Animation
struct ElasticRevealConfig;
impl ElasticRevealConfig {
    const TIME_PER_CHAR: f32 = 0.04;
}

// Bounce In Animation
struct BounceConfig;
impl BounceConfig {
    const DURATION: f32 = 1.3;
}

// Staggered Slide Animation
struct StaggeredSlideConfig;
impl StaggeredSlideConfig {
    const CHAR_DELAY: f32 = 0.05;
    const BASE_DURATION: f32 = 0.5;
    const SLIDE_DISTANCE_PX: f32 = 100.0;
}

// Cascade Zoom Animation
struct CascadeZoomConfig;
impl CascadeZoomConfig {
    const IN_DURATION: f32 = 0.55;
    const OUT_DURATION: f32 = 0.45;
    const CHAR_DELAY: f32 = 0.05;
    const MIN_SCALE: f32 = 0.05;
    const TARGET_SCALE: f32 = 1.0;
    const MAX_SCALE: f32 = 1.25;
    const OVERSHOOT: f32 = 0.22;
}

// Legacy constants for backwards compatibility (to be replaced gradually)
const BASE_TEXT_SCALE: f32 = TextLayoutConfig::BASE_SCALE;
const DESIGN_WIDTH: f32 = TextLayoutConfig::DESIGN_WIDTH;
const MIN_VIEWPORT_SCALE: f32 = TextLayoutConfig::MIN_VIEWPORT_SCALE;
const MAX_VIEWPORT_SCALE: f32 = TextLayoutConfig::MAX_VIEWPORT_SCALE;
const SWING_FREQUENCY: f32 = SwingConfig::FREQUENCY;
const SWING_AMPLITUDE_PX: f32 = SwingConfig::AMPLITUDE_H_PX;
const SWING_VERTICAL_AMPLITUDE_PX: f32 = SwingConfig::AMPLITUDE_V_PX;
const SWING_PHASE_OFFSET: f32 = SwingConfig::PHASE_OFFSET;
const BACKGROUND_OVERHANG_PX: f32 = TextLayoutConfig::BACKGROUND_OVERHANG_PX;
const BREATH_FREQUENCY: f32 = VisualEffectsConfig::BREATH_FREQUENCY;
const BREATH_AMPLITUDE: f32 = VisualEffectsConfig::BREATH_AMPLITUDE;
const PULSE_SCALING: f32 = VisualEffectsConfig::PULSE_SCALING;
const STARTUP_FADE_DURATION: f32 = VisualEffectsConfig::STARTUP_FADE_DURATION;
const SIMPLE_FADE_DURATION: f32 = SimpleFadeConfig::DURATION;
const ELASTIC_REVEAL_TIME_PER_CHAR: f32 = ElasticRevealConfig::TIME_PER_CHAR;
const BOUNCE_DURATION: f32 = BounceConfig::DURATION;
const STAGGERED_CHAR_DELAY: f32 = StaggeredSlideConfig::CHAR_DELAY;
const STAGGERED_BASE_DURATION: f32 = StaggeredSlideConfig::BASE_DURATION;
const SLIDE_DISTANCE_PX: f32 = StaggeredSlideConfig::SLIDE_DISTANCE_PX;
const CASCADE_IN_DURATION: f32 = CascadeZoomConfig::IN_DURATION;
const CASCADE_OUT_DURATION: f32 = CascadeZoomConfig::OUT_DURATION;
const CASCADE_CHAR_DELAY: f32 = CascadeZoomConfig::CHAR_DELAY;
const CASCADE_MIN_SCALE: f32 = CascadeZoomConfig::MIN_SCALE;
const CASCADE_TARGET_SCALE: f32 = CascadeZoomConfig::TARGET_SCALE;
const CASCADE_MAX_SCALE: f32 = CascadeZoomConfig::MAX_SCALE;
const CASCADE_OVERSHOOT: f32 = CascadeZoomConfig::OVERSHOOT;

// === Material + Uniforms =====================================================

#[derive(AsBindGroup, TypePath, Debug, Clone, Asset, Default)]
pub struct CubeFacesMaterial {
    #[uniform(0)]
    params: CubeParams,
}
impl Material2d for CubeFacesMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path("shaders/oldschool.wgsl".into())
    }
}

#[derive(AsBindGroup, TypePath, Debug, Clone, Asset)]
pub struct CrtPostMaterial {
    #[texture(0)]
    #[sampler(1)]
    scene_texture: Handle<Image>,
    #[uniform(2)]
    params: CrtParams,
}
impl Material2d for CrtPostMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path("shaders/crt_post.wgsl".into())
    }
}

#[derive(AsBindGroup, TypePath, Debug, Clone, Asset)]
pub struct LogoMaterial {
    #[texture(0)]
    #[sampler(1)]
    logo_texture: Handle<Image>,
    #[uniform(2)]
    params: LogoShaderParams,
}
impl Material2d for LogoMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path("shaders/logo_wave.wgsl".into())
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

#[derive(ShaderType, Clone, Copy, Debug)]
struct LogoShaderParams {
    time: f32,
    amp_x: f32,
    freq_x: f32,
    speed_x: f32,
    amp_y: f32,
    freq_y: f32,
    speed_y: f32,
}
impl Default for LogoShaderParams {
    fn default() -> Self {
        Self {
            time: 0.0,
            amp_x: LogoConfig::DISTORT_X_AMPLITUDE,
            freq_x: LogoConfig::DISTORT_X_FREQUENCY,
            speed_x: LogoConfig::DISTORT_X_SPEED,
            amp_y: LogoConfig::DISTORT_Y_AMPLITUDE,
            freq_y: LogoConfig::DISTORT_Y_FREQUENCY,
            speed_y: LogoConfig::DISTORT_Y_SPEED,
        }
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

#[derive(ShaderType, Clone, Copy, Debug)]
struct CrtParams {
    time: f32,
    width: f32,
    height: f32,
    crt_enabled: u32,
}
impl Default for CrtParams {
    fn default() -> Self {
        Self {
            time: 0.0,
            width: 1280.0,
            height: 720.0,
            crt_enabled: 1,
        }
    }
}

#[derive(Resource)]
struct MaterialHandle(Handle<CubeFacesMaterial>);

#[derive(Resource)]
struct CrtMaterialHandle(Handle<CrtPostMaterial>);

#[derive(Resource)]
struct LogoMaterialHandle(Handle<LogoMaterial>);

#[derive(Resource, Clone)]
struct SceneRenderTarget(pub Handle<Image>);

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

#[derive(Component)]
struct SurfaceQuad;

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
const YM_TRACK_PATH: &str = "music/Prelude.ym";
const LOGO_TEXTURE_PATH: &str = "textures/vectronix.png";
const MEGA_FONT_LAYOUT: [&str; 3] = [
    " !\"#$%&'()*+,-./0123",
    "456789:;<=>?@ABCDEFG",
    "HIJKLMNOPQRSTUVWXYZ[",
];
const SCROLL_TEXT: &str = "VECTRONIX  YM2149  RUST POWER  BEVY DEMOSCENE  ";
const SCROLL_TEXTURE_WIDTH: u32 = 2048;
const SCROLL_TEXTURE_HEIGHT: u32 = 64;
const SCROLL_GLYPH_WIDTH: u32 = 32;
const SCROLL_GLYPH_HEIGHT: u32 = 32;
const SCROLL_LETTER_SPACING: u32 = 6;
const SCROLL_SPEED_PX: f32 = 220.0;
const SCROLL_HOVER_AMPLITUDE: f32 = 6.0;
const SCROLL_HOVER_FREQUENCY: f32 = 1.4;
const SCROLL_BASE_OFFSET: f32 = 40.0;

struct LogoConfig;
impl LogoConfig {
    const WIDTH: f32 = 1020.0;
    const HEIGHT: f32 = 400.0;
    const TOP_MARGIN_PX: f32 = 40.0;
    const BOTTOM_MARGIN_PX: f32 = 60.0;
    const VERTICAL_CENTER_RATIO: f32 = 0.75;
    const VERTICAL_BIAS_PX: f32 = 20.0;
    const HOVER_AMPLITUDE_X: f32 = 120.0;
    const HOVER_AMPLITUDE_Y: f32 = 0.8; // scaler, actual amplitude computed dynamically
    const HOVER_FREQUENCY_X: f32 = 1.6;
    const HOVER_FREQUENCY_Y: f32 = 2.2;
    const PHASE_DELTA: f32 = std::f32::consts::FRAC_PI_2;
    const DISTORT_X_AMPLITUDE: f32 = 0.016;
    const DISTORT_X_FREQUENCY: f32 = 5.8;
    const DISTORT_X_SPEED: f32 = 1.2;
    const DISTORT_Y_AMPLITUDE: f32 = 0.16;
    const DISTORT_Y_FREQUENCY: f32 = 10.5;
    const DISTORT_Y_SPEED: f32 = 2.9;
}

#[derive(Component)]
struct LogoHover {
    phase: Vec2,
}
impl Default for LogoHover {
    fn default() -> Self {
        Self {
            phase: Vec2::new(0.0, LogoConfig::PHASE_DELTA),
        }
    }
}

#[derive(Component)]
struct LogoQuad;

#[derive(Clone, Copy)]
struct GlyphCoord {
    col: usize,
    row: usize,
}

#[derive(Clone, Copy)]
struct GlyphRect {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

#[derive(Resource)]
struct MegadethFont {
    glyphs: HashMap<char, GlyphCoord>,
    columns: usize,
    rows: usize,
    pixels: Vec<u8>,
    width: u32,
    height: u32,
}
impl MegadethFont {
    fn from_pixels(pixels: Vec<u8>, width: u32, height: u32) -> Self {
        let rows = MEGA_FONT_LAYOUT.len();
        let columns = MEGA_FONT_LAYOUT
            .first()
            .map(|line| line.chars().count())
            .unwrap_or(0);
        let mut glyphs = HashMap::new();
        for (row, line) in MEGA_FONT_LAYOUT.iter().enumerate() {
            for (col, ch) in line.chars().enumerate() {
                glyphs.insert(ch, GlyphCoord { col, row });
            }
        }
        Self {
            glyphs,
            columns,
            rows,
            pixels,
            width,
            height,
        }
    }

    fn coord_for(&self, ch: char) -> GlyphCoord {
        self.glyphs
            .get(&ch)
            .copied()
            .unwrap_or(GlyphCoord { col: 0, row: 0 })
    }
}

#[derive(Resource)]
struct ScrollTextState {
    buffer: Handle<Image>,
    message: Vec<char>,
    start_index: usize,
    offset: f32,
    speed: f32,
    glyph_width: u32,
    glyph_height: u32,
    letter_spacing: u32,
}

#[derive(Component)]
struct ScrollSprite;

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
    CascadeZoom,    // Per-character staggered zoom in/out
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
                    animation: AnimationType::CascadeZoom,
                },
                PushOverlayText {
                    text: "VECTRONIX PRESENTS".into(),
                    cps: 38.0,
                    dwell: 1.2,
                    fade_out: 0.6,
                    animation: AnimationType::CascadeZoom,
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
                    text: "PRESS C TO TOGGLE CRT MODE".into(),
                    cps: 40.0,
                    dwell: 1.2,
                    fade_out: 0.5,
                    animation: AnimationType::ElasticReveal,
                },
                PushOverlayText {
                    text: "PRESS F TO TOGGLE FULLSCREEN MODE".into(),
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
                    animation: AnimationType::CascadeZoom,
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
    scale: f32,               // For BounceIn
    x_offset: f32,            // For StaggeredSlide horizontal movement
    y_offset: f32,            // For vertical animations
    swing_h: f32,             // Horizontal swing offset (calculated by apply_swing_animation)
    swing_v: f32,             // Vertical swing offset (calculated by apply_swing_animation)
    bg_swing_h: f32,          // Background horizontal swing offset
    bg_swing_v: f32,          // Background vertical swing offset
    viewport_scale: f32, // Responsive scale based on window width (calculated by apply_swing_animation)
    cascade_scales: Vec<f32>, // Per-character scale buffer for CascadeZoom
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
            swing_h: 0.0,
            swing_v: 0.0,
            bg_swing_h: 0.0,
            bg_swing_v: 0.0,
            viewport_scale: 1.0, // Default to 1.0x at design width (1280px)
            cascade_scales: Vec::new(),
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
            Material2dPlugin::<CrtPostMaterial>::default(),
            Material2dPlugin::<LogoMaterial>::default(),
            Ym2149Plugin::default(),
            Ym2149VizPlugin::default(),
        ))
        .add_message::<PushOverlayText>()
        .add_systems(
            Startup,
            (
                setup,
                setup_text_overlay,
                setup_logo_mesh,
                setup_scroll_text,
                setup_startup_fade,
                init_resources,
            ),
        )
        .add_systems(
            Update,
            (
                sync_render_target_image,
                spawn_surface_when_ready,
                update_surface_scale_on_resize,
                update_startup_fade,
                toggle_fullscreen,
                toggle_crt,
                exit_on_escape,
                handle_push_events,
                feed_overlay_script,
                apply_swing_animation, // Calculate swing offsets (updates state)
                update_uniforms,
                typewriter_update, // Apply text animation + swing offsets to text layout
                apply_background_swing, // Apply background swing offsets
                animate_logo_hover, // Hover animation for the logo
                update_logo_material, // Update shader uniforms for logo distortion
                update_scroll_buffer,
                animate_scroll_sprite,
            ),
        )
        .run();
}

// === Setup ==================================================================

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    commands.insert_resource(Ym2149Settings {
        loop_enabled: true,
        ..Default::default()
    });

    if PLAY_MUSIC {
        let ym_handle: Handle<Ym2149AudioSource> = asset_server.load(YM_TRACK_PATH);
        let mut playback = Ym2149Playback::from_asset(ym_handle);
        playback.play();
        commands.spawn(playback);
    }

    // Fullscreen Quad (deferred spawn)
    let mesh_handle = meshes.add(Mesh::from(Rectangle::new(2.0, 2.0)));

    let window_size = windows
        .single()
        .map(|window| Vec2::new(window.resolution.width(), window.resolution.height()))
        .unwrap_or(Vec2::new(1280.0, 720.0));

    let extent = Extent3d {
        width: window_size.x.max(1.0).round() as u32,
        height: window_size.y.max(1.0).round() as u32,
        depth_or_array_layers: 1,
    };
    let mut render_target_image = Image {
        texture_descriptor: TextureDescriptor {
            label: Some("demoscene.offscreen".into()),
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
    let render_target_handle = images.add(render_target_image);
    commands.insert_resource(SceneRenderTarget(render_target_handle.clone()));

    commands.spawn((
        Camera2d,
        Camera {
            target: RenderTarget::from(render_target_handle.clone()),
            order: -1,
            clear_color: ClearColorConfig::Custom(Color::BLACK),
            ..default()
        },
        RenderLayers::layer(0),
        Name::new("OffscreenSceneCamera"),
    ));

    commands.spawn((
        Camera2d,
        Camera {
            order: 0,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        RenderLayers::layer(1),
        IsDefaultUiCamera,
        Name::new("DisplayCamera"),
    ));

    let quad_scale = Vec3::new(window_size.x * 0.5, window_size.y * 0.5, 1.0);

    commands.insert_resource(PendingSurface {
        mesh: mesh_handle,
        scale: quad_scale,
        spawned: false,
    });

    // Shader Hot Reload
    let shader_handle: Handle<Shader> = asset_server.load("shaders/oldschool.wgsl");
    commands.insert_resource(StartupFade {
        shader: shader_handle,
        state: StartupFadePhase::Loading,
        timer: 0.0,
        duration: STARTUP_FADE_DURATION,
    });

    spawn_spectrum_bars(&mut commands);
}
fn spawn_spectrum_bars(commands: &mut Commands) {
    const CHANNEL_COUNT: usize = 3;
    const BIN_COUNT: usize = 16;

    let spawn_row = |commands: &mut Commands, is_top: bool| {
        let mut container = Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Px(96.0),
            justify_content: JustifyContent::Center,
            align_items: if is_top {
                AlignItems::FlexStart
            } else {
                AlignItems::FlexEnd
            },
            column_gap: Val::Px(36.0),
            padding: UiRect::axes(Val::Px(18.0), Val::Px(12.0)),
            ..default()
        };
        container.left = Val::Px(0.0);
        container.right = Val::Px(0.0);
        if is_top {
            container.top = Val::Px(18.0);
        } else {
            container.bottom = Val::Px(18.0);
        }

        commands
            .spawn((
                container,
                BackgroundColor(Color::srgba(0.02, 0.03, 0.07, 0.35)),
                RenderLayers::layer(1),
            ))
            .with_children(|row| {
                for channel in 0..CHANNEL_COUNT {
                    row.spawn((
                        Node {
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(4.0),
                            align_items: if is_top {
                                AlignItems::FlexStart
                            } else {
                                AlignItems::FlexEnd
                            },
                            justify_content: JustifyContent::Center,
                            width: Val::Px(240.0),
                            height: Val::Percent(100.0),
                            padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.06, 0.07, 0.12, 0.35)),
                        RenderLayers::layer(1),
                    ))
                    .with_children(|bars| {
                        for bin in 0..BIN_COUNT {
                            bars.spawn((
                                Node {
                                    width: Val::Px(12.0),
                                    height: Val::Px(6.0),
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(0.08, 0.11, 0.18, 0.85)),
                                SpectrumBar { channel, bin },
                                RenderLayers::layer(1),
                            ));
                        }
                    });
                }
            });
    };

    spawn_row(commands, true);
    spawn_row(commands, false);
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
            RenderLayers::layer(1),
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
                RenderLayers::layer(1),
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
                RenderLayers::layer(1),
                Name::new("MainBitmapText"),
            ));
        });
}
fn setup_logo_mesh(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<LogoMaterial>>,
) {
    let mesh_handle = meshes.add(Mesh::from(Rectangle::new(
        LogoConfig::WIDTH,
        LogoConfig::HEIGHT,
    )));
    let texture = asset_server.load(LOGO_TEXTURE_PATH);
    let material_handle = materials.add(LogoMaterial {
        logo_texture: texture,
        params: LogoShaderParams::default(),
    });
    commands.insert_resource(LogoMaterialHandle(material_handle.clone()));

    commands.spawn((
        Mesh2d(mesh_handle),
        MeshMaterial2d(material_handle),
        Transform::from_xyz(0.0, 0.0, 50.0),
        Visibility::Visible,
        InheritedVisibility::VISIBLE,
        ViewVisibility::default(),
        LogoHover::default(),
        LogoQuad,
        RenderLayers::layer(1),
        Name::new("DemosceneLogoQuad"),
    ));
}
fn setup_scroll_text(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let font_path = format!("{}/fonts/megadeth_font.png", ASSET_BASE);
    let font_bytes = fs::read(&font_path).expect("failed to read megadeth font png");
    let decoded = image::load_from_memory(&font_bytes)
        .expect("failed to decode megadeth font png")
        .to_rgba8();
    let (font_width, font_height) = decoded.dimensions();
    let font = MegadethFont::from_pixels(decoded.into_raw(), font_width, font_height);

    let extent = Extent3d {
        width: SCROLL_TEXTURE_WIDTH,
        height: SCROLL_TEXTURE_HEIGHT,
        depth_or_array_layers: 1,
    };
    let scroll_image = Image::new_fill(
        extent,
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    let buffer_handle = images.add(scroll_image);

    commands.insert_resource(font);
    commands.insert_resource(ScrollTextState {
        buffer: buffer_handle.clone(),
        message: SCROLL_TEXT.chars().collect(),
        start_index: 0,
        offset: 0.0,
        speed: SCROLL_SPEED_PX,
        glyph_width: SCROLL_GLYPH_WIDTH,
        glyph_height: SCROLL_GLYPH_HEIGHT,
        letter_spacing: SCROLL_LETTER_SPACING,
    });

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(SCROLL_BASE_OFFSET),
                height: Val::Px(SCROLL_TEXTURE_HEIGHT as f32),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            Visibility::Visible,
            InheritedVisibility::VISIBLE,
            ScrollSprite,
            RenderLayers::layer(1),
            Name::new("ScrollTextContainer"),
        ))
        .with_children(|parent| {
            parent.spawn((
                ImageNode::from(buffer_handle.clone()).with_mode(NodeImageMode::Stretch),
                Node {
                    width: Val::Px(SCROLL_TEXTURE_WIDTH as f32),
                    height: Val::Px(SCROLL_TEXTURE_HEIGHT as f32),
                    ..default()
                },
                Visibility::Visible,
                InheritedVisibility::VISIBLE,
                Name::new("ScrollTextImage"),
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
        RenderLayers::layer(1),
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

fn sync_render_target_image(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut images: ResMut<Assets<Image>>,
    render_target: Option<Res<SceneRenderTarget>>,
) {
    let Some(render_target) = render_target else {
        return;
    };
    let Ok(window) = windows.single() else {
        return;
    };
    let Some(image) = images.get_mut(&render_target.0) else {
        return;
    };

    let scale_factor = window.resolution.scale_factor() as f32;
    let desired_width = (window.resolution.width() * scale_factor).round().max(1.0) as u32;
    let desired_height = (window.resolution.height() * scale_factor).round().max(1.0) as u32;

    let current_size = image.texture_descriptor.size;
    if current_size.width != desired_width || current_size.height != desired_height {
        let extent = Extent3d {
            width: desired_width,
            height: desired_height,
            depth_or_array_layers: 1,
        };
        image.resize(extent);
    }
}

fn spawn_surface_when_ready(
    asset_server: Res<AssetServer>,
    pending: Option<ResMut<PendingSurface>>,
    mut materials: ResMut<Assets<CubeFacesMaterial>>,
    mut crt_materials: ResMut<Assets<CrtPostMaterial>>,
    mut commands: Commands,
    fade: Res<StartupFade>,
    render_target: Res<SceneRenderTarget>,
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
        SurfaceQuad,
        Visibility::default(),
        InheritedVisibility::default(),
        ViewVisibility::default(),
        RenderLayers::layer(0),
        Name::new("CubeFacesSurface"),
    ));
    commands.insert_resource(MaterialHandle(material_handle));

    let crt_material_handle = crt_materials.add(CrtPostMaterial {
        scene_texture: render_target.0.clone(),
        params: CrtParams::default(),
    });
    commands.spawn((
        Mesh2d(pending.mesh.clone()),
        MeshMaterial2d(crt_material_handle.clone()),
        Transform::from_scale(pending.scale),
        SurfaceQuad,
        Visibility::default(),
        InheritedVisibility::default(),
        ViewVisibility::default(),
        RenderLayers::layer(1),
        Name::new("CrtPostSurface"),
    ));
    commands.insert_resource(CrtMaterialHandle(crt_material_handle));

    pending.spawned = true;
}

fn toggle_fullscreen(
    keys: Res<ButtonInput<KeyCode>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    if !keys.just_pressed(KeyCode::KeyF) {
        return;
    }

    if let Ok(mut window) = windows.single_mut() {
        let is_fullscreen = matches!(
            window.mode,
            WindowMode::BorderlessFullscreen(_) | WindowMode::Fullscreen(_, _)
        );
        window.mode = if is_fullscreen {
            WindowMode::Windowed
        } else {
            WindowMode::BorderlessFullscreen(MonitorSelection::Current)
        };
    }
}

fn toggle_crt(keys: Res<ButtonInput<KeyCode>>, mut crt: ResMut<CrtState>) {
    if keys.just_pressed(KeyCode::KeyC) {
        crt.enabled = !crt.enabled;
    }
}

fn update_surface_scale_on_resize(
    mut resize_events: MessageReader<WindowResized>,
    mut surfaces: Query<&mut Transform, With<SurfaceQuad>>,
    pending: Option<ResMut<PendingSurface>>,
) {
    let mut has_resize = false;
    let mut latest_scale = Vec3::ONE;
    for event in resize_events.read() {
        latest_scale = Vec3::new(event.width * 0.5, event.height * 0.5, 1.0);
        has_resize = true;
    }

    if !has_resize {
        return;
    }

    for mut transform in surfaces.iter_mut() {
        transform.scale = latest_scale;
    }

    if let Some(mut pending) = pending {
        pending.scale = latest_scale;
    }
}

fn exit_on_escape(keys: Res<ButtonInput<KeyCode>>, mut exit: MessageWriter<AppExit>) {
    if keys.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::default());
    }
}

// === Uniform Update =========================================================

fn update_uniforms(
    time: Res<Time>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut materials: ResMut<Assets<CubeFacesMaterial>>,
    mut crt_materials: ResMut<Assets<CrtPostMaterial>>,
    mat: Option<Res<MaterialHandle>>,
    crt_mat: Option<Res<CrtMaterialHandle>>,
    crt: Option<Res<CrtState>>,
) {
    let Some(mat) = mat else {
        return;
    };
    let Ok(window) = windows.single() else {
        return;
    };
    let Some(material) = materials.get_mut(&mat.0) else {
        return;
    };

    let crt_enabled_flag = if let Some(crt_state) = crt.as_ref() {
        if crt_state.enabled { 1 } else { 0 }
    } else {
        1
    };

    let scale_factor = window.resolution.scale_factor() as f32;
    let physical_width = (window.resolution.width() * scale_factor).round().max(1.0);
    let physical_height = (window.resolution.height() * scale_factor).round().max(1.0);

    material.params.time = time.elapsed_secs();
    material.params.width = physical_width;
    material.params.height = physical_height;
    material.params.frame = material.params.frame.wrapping_add(1);
    material.params.crt_enabled = crt_enabled_flag;

    if let Some(crt_mat) = crt_mat {
        if let Some(crt_material) = crt_materials.get_mut(&crt_mat.0) {
            crt_material.params.time = time.elapsed_secs();
            crt_material.params.width = physical_width;
            crt_material.params.height = physical_height;
            crt_material.params.crt_enabled = crt_enabled_flag;
        }
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
                    AnimationType::CascadeZoom => {
                        // Characters scale in sequentially from tiny to oversized
                        if state.cascade_scales.len() != total {
                            state.cascade_scales.resize(total, CASCADE_MIN_SCALE);
                        }
                        state.visible_chars = total;
                        let timer = state.timer;
                        let cascade_total = ((total.saturating_sub(1) as f32) * CASCADE_CHAR_DELAY
                            + CASCADE_IN_DURATION)
                            .max(0.0001);
                        let global_t = (timer / cascade_total).clamp(0.0, 1.0);
                        let global_bounce = ease_out_bounce(global_t);
                        let global_overshoot = CASCADE_OVERSHOOT * (1.0 - global_t) * global_bounce;
                        state.scale = (CASCADE_MIN_SCALE
                            + global_bounce * (CASCADE_TARGET_SCALE - CASCADE_MIN_SCALE)
                            + global_overshoot)
                            .clamp(CASCADE_MIN_SCALE, CASCADE_MAX_SCALE);
                        state.alpha = 1.0;
                        for (i, scale) in state.cascade_scales.iter_mut().enumerate() {
                            let start_time = i as f32 * CASCADE_CHAR_DELAY;
                            let local_t =
                                ((timer - start_time) / CASCADE_IN_DURATION).clamp(0.0, 1.0);
                            if local_t <= 0.0 {
                                *scale = CASCADE_MIN_SCALE;
                                continue;
                            }
                            let bounce = ease_out_bounce(local_t);
                            let overshoot = CASCADE_OVERSHOOT * (1.0 - local_t) * bounce;
                            let raw_scale = CASCADE_MIN_SCALE
                                + bounce * (CASCADE_TARGET_SCALE - CASCADE_MIN_SCALE)
                                + overshoot;
                            *scale = raw_scale.clamp(CASCADE_MIN_SCALE, CASCADE_MAX_SCALE);
                        }
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
                        AnimationType::CascadeZoom => {
                            (total.saturating_sub(1) as f32) * CASCADE_CHAR_DELAY
                                + CASCADE_IN_DURATION
                        }
                    }
                {
                    state.visible_chars = total;
                    state.phase = Phase::Dwell;
                    state.timer = 0.0;
                    state.scale = 1.0;
                    state.x_offset = 0.0;
                    state.y_offset = 0.0;
                    if state.animation_type == AnimationType::CascadeZoom {
                        for scale in state.cascade_scales.iter_mut() {
                            *scale = CASCADE_TARGET_SCALE;
                        }
                    }
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
                let total_chars = current.text.chars().count();
                if state.animation_type == AnimationType::CascadeZoom
                    && state.cascade_scales.len() != total_chars
                {
                    state
                        .cascade_scales
                        .resize(total_chars, CASCADE_TARGET_SCALE);
                }
                let fade_total = match state.animation_type {
                    AnimationType::CascadeZoom => {
                        let cascade_total = (state.cascade_scales.len().saturating_sub(1) as f32)
                            * CASCADE_CHAR_DELAY
                            + CASCADE_OUT_DURATION;
                        current.fade_out.max(cascade_total)
                    }
                    _ => current.fade_out,
                }
                .max(0.0001);
                let t = (state.timer / fade_total).clamp(0.0, 1.0);

                // Apply fade-out animation based on type
                match state.animation_type {
                    AnimationType::CascadeZoom => {
                        let timer = state.timer;
                        for (i, scale) in state.cascade_scales.iter_mut().enumerate() {
                            let start_time = i as f32 * CASCADE_CHAR_DELAY;
                            let local_t =
                                ((timer - start_time) / CASCADE_OUT_DURATION).clamp(0.0, 1.0);
                            if local_t <= 0.0 {
                                continue;
                            }
                            let eased = ease_out_cubic(local_t);
                            let raw = CASCADE_TARGET_SCALE * (1.0 - eased);
                            *scale = raw.clamp(0.0, CASCADE_TARGET_SCALE);
                        }
                        state.alpha = 1.0 - t;
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
                    state.cascade_scales.clear();
                    return;
                }
            }
            Phase::Idle => {}
        }

        let use_cascade = state.animation_type == AnimationType::CascadeZoom;
        let visible: String = if use_cascade {
            current.text.clone()
        } else {
            current.text.chars().take(state.visible_chars).collect()
        };
        if let Ok((mut image, mut layout)) = main_query.single_mut() {
            let (glyph_size, texture_size) = if use_cascade {
                let metrics = rebuild_bitmap_text_cascade(
                    &visible,
                    &font,
                    &image.image,
                    &mut images,
                    &state.cascade_scales,
                    CASCADE_TARGET_SCALE,
                    CASCADE_MAX_SCALE,
                );
                (metrics.rest, metrics.texture)
            } else {
                let base = rebuild_bitmap_text(&visible, &font, &image.image, &mut images);
                (base, base)
            };
            let width_correction = if glyph_size.x > 0 {
                texture_size.x as f32 / glyph_size.x as f32
            } else {
                1.0
            };
            let height_correction = if glyph_size.y > 0 {
                texture_size.y as f32 / glyph_size.y as f32
            } else {
                1.0
            };
            let zoom = if matches!(state.phase, Phase::FadeOut) {
                state.alpha.clamp(0.0, 1.0)
            } else {
                1.0
            };

            // Apply scale for animation types that use it
            let scale_factor = match state.animation_type {
                AnimationType::BounceIn | AnimationType::CascadeZoom => state.scale,
                _ => 1.0,
            };

            // Final text dimensions: base * zoom * animation_scale * base_scale * viewport_scale
            let width = (glyph_size.x as f32
                * width_correction
                * zoom
                * scale_factor
                * BASE_TEXT_SCALE
                * state.viewport_scale)
                .max(0.0);
            let height = (glyph_size.y as f32
                * height_correction
                * zoom
                * scale_factor
                * BASE_TEXT_SCALE
                * state.viewport_scale)
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

            let (r, g, b) = (
                (brightness + cool_shift).clamp(0.0, 1.0),
                (brightness + cool_shift * 0.5).clamp(0.0, 1.0),
                1.0,
            );

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
        state.cascade_scales.clear();

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
            AnimationType::CascadeZoom => {
                let total = state
                    .current
                    .as_ref()
                    .map(|m| m.text.chars().count())
                    .unwrap_or(0);
                state.cascade_scales.resize(total, CASCADE_MIN_SCALE);
                state.alpha = 1.0;
                state.scale = CASCADE_MIN_SCALE;
                state.visible_chars = total;
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
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    let elapsed = time.elapsed_secs();

    // Get actual window width and calculate responsive scales
    let viewport_scale = windows
        .single()
        .map(|window| {
            let logical_width = window.resolution.width();
            let scale_factor = window.resolution.scale_factor() as f32;
            let physical_width = logical_width * scale_factor;
            (physical_width / DESIGN_WIDTH).clamp(MIN_VIEWPORT_SCALE, MAX_VIEWPORT_SCALE)
        })
        .unwrap_or(1.0);

    // Calculate swing amplitudes responsive to window width
    let swing_h_amplitude = SWING_AMPLITUDE_PX * viewport_scale;
    let swing_v_amplitude = SWING_VERTICAL_AMPLITUDE_PX * viewport_scale;

    // Calculate viewport scale for responsive text sizing
    // Scales between 0.8x and 1.5x based on window width relative to design width
    state.viewport_scale = viewport_scale;

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

fn animate_logo_hover(
    time: Res<Time>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut query: Query<(&mut Transform, &LogoHover)>,
) {
    let elapsed = time.elapsed_secs();
    let (_window_height, half_height) = match windows.single() {
        Ok(window) => {
            let h = window.resolution.height();
            (h, h * 0.5)
        }
        Err(_) => {
            let h = 720.0;
            (h, h * 0.5)
        }
    };
    for (mut transform, hover) in query.iter_mut() {
        let horizontal = (elapsed * LogoConfig::HOVER_FREQUENCY_X + hover.phase.x).sin()
            * LogoConfig::HOVER_AMPLITUDE_X;
        let top_limit = half_height - LogoConfig::TOP_MARGIN_PX - LogoConfig::HEIGHT * 0.5;
        let bottom_limit = -half_height + LogoConfig::BOTTOM_MARGIN_PX + LogoConfig::HEIGHT * 0.5;
        let span = (top_limit - bottom_limit).max(10.0);
        let mut center =
            bottom_limit + span * LogoConfig::VERTICAL_CENTER_RATIO + LogoConfig::VERTICAL_BIAS_PX;
        center = center.clamp(bottom_limit + 20.0, top_limit - 20.0);
        let amplitude = ((top_limit - center).min(center - bottom_limit)).max(10.0)
            * LogoConfig::HOVER_AMPLITUDE_Y;
        let swing = (elapsed * LogoConfig::HOVER_FREQUENCY_Y + hover.phase.y).sin() * amplitude;
        let y = (center + swing).clamp(bottom_limit, top_limit);
        transform.translation.x = horizontal;
        transform.translation.y = y;
    }
}

fn update_logo_material(
    time: Res<Time>,
    mut materials: ResMut<Assets<LogoMaterial>>,
    handle: Option<Res<LogoMaterialHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    if let Some(material) = materials.get_mut(&handle.0) {
        material.params.time = time.elapsed_secs();
    }
}

fn update_scroll_buffer(
    time: Res<Time>,
    font: Option<Res<MegadethFont>>,
    state: Option<ResMut<ScrollTextState>>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(font) = font else {
        return;
    };
    let Some(mut state) = state else {
        return;
    };
    if state.message.is_empty() {
        return;
    }

    let advance = (state.glyph_width + state.letter_spacing) as f32;
    state.offset += state.speed * time.delta_secs();
    while state.offset >= advance {
        state.offset -= advance;
        state.start_index = (state.start_index + 1) % state.message.len();
    }

    let Some(buffer) = images.get_mut(&state.buffer) else {
        return;
    };
    let tex_width = buffer.texture_descriptor.size.width;
    let tex_height = buffer.texture_descriptor.size.height;
    let Some(pixels) = buffer.data.as_mut() else {
        return;
    };
    for chunk in pixels.chunks_exact_mut(4) {
        chunk[0] = 0;
        chunk[1] = 0;
        chunk[2] = 0;
        chunk[3] = 0;
    }

    let baseline = ((tex_height.saturating_sub(state.glyph_height)) / 2) as i32;
    let advance_i = (state.glyph_width + state.letter_spacing) as i32;
    let mut draw_x = -(state.offset.round() as i32);
    let mut idx = state.start_index;
    while draw_x < tex_width as i32 {
        let ch = state.message[idx];
        let rect = glyph_rect_for(&font, ch);
        blit_scaled_glyph(
            &font.pixels,
            font.width,
            rect,
            pixels,
            tex_width,
            tex_height,
            draw_x,
            baseline,
            state.glyph_width,
            state.glyph_height,
        );
        draw_x += advance_i;
        idx = (idx + 1) % state.message.len();
    }
}

fn animate_scroll_sprite(
    time: Res<Time>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut query: Query<&mut Node, With<ScrollSprite>>,
) {
    let elapsed = time.elapsed_secs();
    let base = windows
        .single()
        .map(|window| window.resolution.height() * 0.05)
        .unwrap_or(SCROLL_BASE_OFFSET);
    let hover = (elapsed * SCROLL_HOVER_FREQUENCY).sin() * SCROLL_HOVER_AMPLITUDE;
    for mut layout in query.iter_mut() {
        layout.bottom = Val::Px(base + hover);
    }
}

/// Prepare font image by converting to RGBA8 format if needed
fn prepare_font_image(font: &BitmapFont, images: &mut Assets<Image>) {
    if let Some(font_image) = images.get_mut(&font.image) {
        if font_image.texture_descriptor.format != TextureFormat::Rgba8UnormSrgb {
            if let Some(converted) = font_image.convert(TextureFormat::Rgba8UnormSrgb) {
                *font_image = converted;
            }
        }
    }
}

/// Get font texture data (width, height, pixels)
fn get_font_data(font: &BitmapFont, images: &Assets<Image>) -> Option<(u32, u32, Vec<u8>)> {
    let font_image = images.get(&font.image)?;
    let font_pixels = font_image.data.clone()?;
    if font_pixels.is_empty() {
        return None;
    }
    Some((
        font_image.texture_descriptor.size.width,
        font_image.texture_descriptor.size.height,
        font_pixels,
    ))
}

fn glyph_rect_for(font: &MegadethFont, ch: char) -> GlyphRect {
    let coord = font.coord_for(ch);
    let cols = font.columns.max(1);
    let rows = font.rows.max(1);
    let cell_w = (font.width / cols as u32).max(1);
    let cell_h = (font.height / rows as u32).max(1);
    let x_base = coord.col as u32 * cell_w;
    let y_base = coord.row as u32 * cell_h;
    let mut min_x = cell_w;
    let mut max_x = 0;
    let mut min_y = cell_h;
    let mut max_y = 0;
    let threshold = 32u8;
    for y in 0..cell_h {
        let sy = y_base + y;
        if sy >= font.height {
            break;
        }
        for x in 0..cell_w {
            let sx = x_base + x;
            if sx >= font.width {
                break;
            }
            let idx = ((sy * font.width + sx) * 4) as usize;
            let r = font.pixels[idx];
            let g = font.pixels[idx + 1];
            let b = font.pixels[idx + 2];
            let a = font.pixels[idx + 3];
            let luminance = ((r as u16 + g as u16 + b as u16) / 3) as u8;
            if a < 16 && luminance < threshold {
                continue;
            }
            if x < min_x {
                min_x = x;
            }
            if x > max_x {
                max_x = x;
            }
            if y < min_y {
                min_y = y;
            }
            if y > max_y {
                max_y = y;
            }
        }
    }
    if max_x < min_x || max_y < min_y {
        return GlyphRect {
            x: x_base,
            y: y_base,
            width: cell_w,
            height: cell_h,
        };
    }
    GlyphRect {
        x: x_base + min_x,
        y: y_base + min_y,
        width: (max_x - min_x + 1).max(1),
        height: (max_y - min_y + 1).max(1),
    }
}

fn blit_scaled_glyph(
    src_pixels: &[u8],
    src_width: u32,
    rect: GlyphRect,
    dst_pixels: &mut [u8],
    dst_width: u32,
    dst_height: u32,
    dest_x: i32,
    dest_y: i32,
    target_width: u32,
    target_height: u32,
) {
    if rect.width == 0 || rect.height == 0 {
        return;
    }
    let scale_x = rect.width as f32 / target_width.max(1) as f32;
    let scale_y = rect.height as f32 / target_height.max(1) as f32;
    for ty in 0..target_height {
        let sy = rect.y
            + ((ty as f32) * scale_y)
                .floor()
                .clamp(0.0, (rect.height - 1) as f32) as u32;
        let dy = dest_y + ty as i32;
        if dy < 0 || dy >= dst_height as i32 {
            continue;
        }
        for tx in 0..target_width {
            let sx = rect.x
                + ((tx as f32) * scale_x)
                    .floor()
                    .clamp(0.0, (rect.width - 1) as f32) as u32;
            let dx = dest_x + tx as i32;
            if dx < 0 || dx >= dst_width as i32 {
                continue;
            }
            let src_index = ((sy * src_width + sx) * 4) as usize;
            let rgba = &src_pixels[src_index..src_index + 4];
            if rgba[3] == 0 {
                continue;
            }
            let luminance = (rgba[0] as u32 + rgba[1] as u32 + rgba[2] as u32) / 3;
            if luminance < 48 {
                continue;
            }
            let dst_index = (((dy as u32) * dst_width + dx as u32) * 4) as usize;
            dst_pixels[dst_index..dst_index + 4].copy_from_slice(rgba);
        }
    }
}

/// Create empty 1x1 transparent image
fn create_empty_image(handle: &Handle<Image>, images: &mut Assets<Image>) {
    if let Some(image) = images.get_mut(handle) {
        image.resize(Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        });
        image.data = Some(vec![0, 0, 0, 0]);
    }
}

fn rebuild_bitmap_text(
    text: &str,
    font: &BitmapFont,
    handle: &Handle<Image>,
    images: &mut Assets<Image>,
) -> UVec2 {
    prepare_font_image(font, images);

    if text.is_empty() {
        create_empty_image(handle, images);
        return UVec2::new(1, 1);
    }

    let glyphs: Vec<char> = text.chars().collect();
    if glyphs.is_empty() {
        return UVec2::ZERO;
    }

    let Some((texture_width, texture_height, font_pixels)) = get_font_data(font, images) else {
        return UVec2::ZERO;
    };

    let Some(image) = images.get_mut(handle) else {
        return UVec2::ZERO;
    };

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

struct CascadeImageMetrics {
    rest: UVec2,
    texture: UVec2,
}

fn rebuild_bitmap_text_cascade(
    text: &str,
    font: &BitmapFont,
    handle: &Handle<Image>,
    images: &mut Assets<Image>,
    scales: &[f32],
    rest_scale: f32,
    max_scale: f32,
) -> CascadeImageMetrics {
    prepare_font_image(font, images);

    if text.is_empty() {
        create_empty_image(handle, images);
        return CascadeImageMetrics {
            rest: UVec2::new(1, 1),
            texture: UVec2::new(1, 1),
        };
    }

    let glyphs: Vec<char> = text.chars().collect();
    if glyphs.is_empty() {
        return CascadeImageMetrics {
            rest: UVec2::ZERO,
            texture: UVec2::ZERO,
        };
    }

    let Some((texture_width, texture_height, font_pixels)) = get_font_data(font, images) else {
        return CascadeImageMetrics {
            rest: UVec2::ZERO,
            texture: UVec2::ZERO,
        };
    };

    let Some(image) = images.get_mut(handle) else {
        return CascadeImageMetrics {
            rest: UVec2::ZERO,
            texture: UVec2::ZERO,
        };
    };
    let cell_w = font.cell_size.x;
    let cell_h = font.cell_size.y;
    let spacing = font.letter_spacing;
    let rest_scale = rest_scale.max(CASCADE_MIN_SCALE);
    let max_scale = max_scale.max(rest_scale);
    let rest_char_w = (cell_w as f32 * rest_scale).ceil().max(1.0) as u32;
    let rest_char_h = (cell_h as f32 * rest_scale).ceil().max(1.0) as u32;
    let max_char_w = (cell_w as f32 * max_scale).ceil().max(rest_char_w as f32) as u32;
    let max_char_h = (cell_h as f32 * max_scale).ceil().max(rest_char_h as f32) as u32;
    let glyph_count = glyphs.len() as u32;

    let width = glyph_count * (max_char_w + spacing) - spacing;
    let height = max_char_h.max(1);
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
        let scale = scales
            .get(index)
            .copied()
            .unwrap_or(CASCADE_MIN_SCALE)
            .clamp(0.0, max_scale);
        if scale <= 0.01 {
            continue;
        }
        let char_w = (cell_w as f32 * scale).ceil().max(1.0) as u32;
        let char_h = (cell_h as f32 * scale).ceil().max(1.0) as u32;
        if char_w == 0 || char_h == 0 {
            continue;
        }

        let dest_x_base = index as u32 * (max_char_w + spacing);
        let dest_x_offset =
            ((max_char_w as i32 - char_w as i32) / 2).clamp(0, max_char_w as i32) as u32;
        let dest_y_offset =
            ((max_char_h as i32 - char_h as i32) / 2).clamp(0, max_char_h as i32) as u32;
        let dest_x = dest_x_base + dest_x_offset;
        let dest_y = dest_y_offset;

        let src_x = (coord.x * cell_w) as usize;
        let src_y = (coord.y * cell_h) as usize;

        for dy in 0..char_h {
            if dest_y + dy >= height {
                break;
            }
            let sample_y = ((dy as f32 / scale).floor() as usize).min(cell_h as usize - 1);
            let src_row = (src_y + sample_y).min(texture_height as usize - 1);

            for dx in 0..char_w {
                if dest_x + dx >= width {
                    break;
                }
                let sample_x = ((dx as f32 / scale).floor() as usize).min(cell_w as usize - 1);
                let src_col = (src_x + sample_x).min(texture_width as usize - 1);
                let src_index = (src_row * row_span + src_col) * pixel_stride;
                let dst_index = (((dest_y + dy) * width + (dest_x + dx)) * 4) as usize;

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
    let rest_width = glyph_count * (rest_char_w + spacing) - spacing;
    CascadeImageMetrics {
        rest: UVec2::new(rest_width.max(1), rest_char_h.max(1)),
        texture: UVec2::new(width.max(1), height.max(1)),
    }
}
