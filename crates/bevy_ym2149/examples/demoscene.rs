//! Cube Faces Raymarch (ShaderToy Buffer A Port, single pass)
//! Mit Text-Overlay + YM2149 Playback.
//!
//! Shader: assets/shaders/cube_faces_singlepass.wgsl

use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::log::debug;
use bevy::math::primitives::Rectangle;
use bevy::{
    asset::AssetPlugin,
    prelude::*,
    render::render_resource::{AsBindGroup, Extent3d, ShaderType, TextureDimension, TextureFormat},
    shader::{Shader, ShaderRef},
    sprite_render::{Material2d, Material2dPlugin, MeshMaterial2d},
    ui::widget::{ImageNode, NodeImageMode},
};
use bevy_mesh::Mesh2d;
use bevy_ym2149::{Ym2149AudioSource, Ym2149Playback, Ym2149Plugin};

// === Material + Uniforms =====================================================

#[derive(AsBindGroup, TypePath, Debug, Clone, Asset)]
pub struct CubeFacesMaterial {
    #[uniform(0)]
    params: CubeParams,
}
impl Default for CubeFacesMaterial {
    fn default() -> Self {
        Self {
            params: CubeParams::default(),
        }
    }
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
}
impl Default for CubeParams {
    fn default() -> Self {
        Self {
            time: 0.0,
            width: 1280.0,
            height: 720.0,
            mouse: Vec4::ZERO,
            frame: 0,
        }
    }
}

#[derive(Resource)]
struct MaterialHandle(Handle<CubeFacesMaterial>);

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

#[derive(Clone, Message)]
pub struct PushOverlayText {
    pub text: String,
    pub cps: f32,
    pub dwell: f32,
    pub fade_out: f32,
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
                    text: "* OLD SKOOL PRESENTS *".into(),
                    cps: 38.0,
                    dwell: 1.2,
                    fade_out: 0.6,
                },
                PushOverlayText {
                    text: "6 SHADERS ON A RAYMARCHED CUBE".into(),
                    cps: 52.0,
                    dwell: 1.3,
                    fade_out: 0.6,
                },
                PushOverlayText {
                    text: "GLENZ - RING - VORONOI - PLASMA - TWIRL - ROOM".into(),
                    cps: 60.0,
                    dwell: 1.5,
                    fade_out: 0.75,
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
}
impl Default for TextWriterState {
    fn default() -> Self {
        Self {
            timer: 0.0,
            visible_chars: 0,
            phase: Phase::Idle,
            current: None,
            alpha: 0.0,
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

#[derive(Message)]
pub struct BeatPulseTrigger;
#[derive(Resource, Default)]
struct BeatPulse {
    energy: f32,
    decay_per_sec: f32,
    add_on_beat: f32,
}
#[derive(Resource)]
struct BpmClock {
    bpm: f32,
    phase: f32,
}

// === App ====================================================================

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Oldskool — Cube Faces Raymarch".into(),
                        resolution: (1280, 720).into(),
                        present_mode: bevy::window::PresentMode::AutoVsync,
                        resizable: true,
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin {
                    file_path: "assets".into(),
                    ..default()
                }),
        )
        .add_plugins((
            Material2dPlugin::<CubeFacesMaterial>::default(),
            Ym2149Plugin::default(),
        ))
        .add_message::<PushOverlayText>()
        .add_message::<BeatPulseTrigger>()
        .add_systems(Startup, (setup, setup_text_overlay, init_resources))
        .add_systems(
            Update,
            (
                bpm_fallback_tick,
                on_beat_trigger,
                beat_pulse_decay,
                handle_push_events,
                feed_overlay_script,
                update_uniforms,
                typewriter_update,
            ),
        )
        .run();
}

// === Setup ==================================================================

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<CubeFacesMaterial>>,
    windows: Query<&Window>,
) {
    commands.spawn(Camera2d);

    if PLAY_MUSIC {
        let ym_handle: Handle<Ym2149AudioSource> = asset_server.load(YM_TRACK_PATH);
        let mut playback = Ym2149Playback::from_asset(ym_handle);
        playback.play();
        commands.spawn(playback);
    }

    // Fullscreen Quad
    let mesh = meshes.add(Mesh::from(Rectangle::new(2.0, 2.0)));

    let window_size = windows
        .iter()
        .next()
        .map(|w| Vec2::new(w.resolution.width(), w.resolution.height()))
        .unwrap_or(Vec2::new(1280.0, 720.0));

    let quad_scale = Vec3::new(window_size.x * 0.5, window_size.y * 0.5, 1.0);

    let material_handle = materials.add(CubeFacesMaterial::default());
    commands.spawn((
        Mesh2d(mesh),
        MeshMaterial2d(material_handle.clone()),
        Transform::from_scale(quad_scale),
        GlobalTransform::default(),
        Visibility::default(),
        Name::new("CubeFacesSurface"),
    ));
    commands.insert_resource(MaterialHandle(material_handle));

    // Shader Hot Reload
    let _ = asset_server.load::<Shader>("shaders/cube_faces_singlepass.wgsl");
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
                    width: Val::Percent(100.0),
                    height: Val::Px(120.0),
                    left: Val::Px(0.0),
                    right: Val::Px(0.0),
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

fn init_resources(mut commands: Commands) {
    commands.insert_resource(TextQueue::default());
    commands.insert_resource(OverlayScript::default());
    commands.insert_resource(TextWriterState::default());
    commands.insert_resource(BeatPulse {
        energy: 0.0,
        decay_per_sec: 2.2,
        add_on_beat: 1.0,
    });
    commands.insert_resource(BpmClock {
        bpm: 120.0,
        phase: 0.0,
    });
}

// === Uniform Update =========================================================

fn update_uniforms(
    time: Res<Time>,
    windows: Query<&Window>,
    mut materials: ResMut<Assets<CubeFacesMaterial>>,
    mat: Res<MaterialHandle>,
) {
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
}

// === Beat Pulse + Overlay Writer ============================================

fn on_beat_trigger(mut ev: MessageReader<BeatPulseTrigger>, mut pulse: ResMut<BeatPulse>) {
    for _ in ev.read() {
        pulse.energy += pulse.add_on_beat;
    }
}
fn beat_pulse_decay(time: Res<Time>, mut pulse: ResMut<BeatPulse>) {
    let dt = time.delta_secs();
    pulse.energy = (pulse.energy - pulse.decay_per_sec * dt).max(0.0);
}
fn bpm_fallback_tick(
    time: Res<Time>,
    mut evw: MessageWriter<BeatPulseTrigger>,
    mut clock: ResMut<BpmClock>,
) {
    let dt = time.delta_secs();
    clock.phase += clock.bpm / 60.0 * dt;
    if clock.phase >= 1.0 {
        clock.phase -= 1.0;
        evw.write(BeatPulseTrigger);
    }
}

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
        let pulse_strength = (pulse.energy * 0.6).clamp(0.0, 1.2);
        let breath = (time.elapsed_secs() * 1.6).sin() * 0.1 + 0.9;

        match state.phase {
            Phase::Typing => {
                state.timer += dt;
                let total = current.text.chars().count();
                state.visible_chars = (state.timer * current.cps).floor() as usize;
                if state.visible_chars >= total {
                    state.visible_chars = total;
                    state.phase = Phase::Dwell;
                    state.timer = 0.0;
                }
                state.alpha = 1.0;
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
                state.alpha = 1.0 - t;
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
            let width = (size.x as f32 * zoom).max(0.0);
            let height = (size.y as f32 * zoom).max(0.0);
            layout.width = Val::Px(width);
            layout.height = Val::Px(height);
            layout.min_width = Val::Px(width);
            layout.min_height = Val::Px(height);
            layout.max_width = Val::Px(width);
            layout.max_height = Val::Px(height);

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
            image.color = Color::srgba(
                (brightness + cool_shift).clamp(0.0, 1.0),
                (brightness + cool_shift * 0.5).clamp(0.0, 1.0),
                1.0,
                state.alpha,
            );
        }
    }
}

fn start_next_message(state: &mut TextWriterState, queue: &mut TextQueue) {
    if let Some(msg) = queue.0.first().cloned() {
        state.current = Some(msg);
        state.timer = 0.0;
        state.visible_chars = 0;
        state.phase = Phase::Typing;
        state.alpha = 1.0;
    } else {
        state.current = None;
        state.phase = Phase::Idle;
        state.alpha = 0.0;
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
