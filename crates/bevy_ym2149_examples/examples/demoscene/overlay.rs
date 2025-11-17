use std::collections::{HashMap, VecDeque};

use bevy::{
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    camera::visibility::RenderLayers,
    ui::{
        AlignItems, AlignSelf, BackgroundColor, FlexDirection, GlobalZIndex, JustifyContent,
        JustifySelf, Node, PositionType, UiRect, Val,
        widget::{ImageNode, NodeImageMode},
    },
    window::PrimaryWindow,
};
use bevy_ym2149_viz::SpectrumBar;

use super::config::{
    BITMAP_CELL_SIZE, BITMAP_FONT_LAYOUT, BITMAP_LETTER_SPACING, BounceConfig, CascadeZoomConfig,
    ElasticRevealConfig, SimpleFadeConfig, StaggeredSlideConfig, SwingConfig, TextLayoutConfig,
    VisualEffectsConfig, ease_out_bounce, ease_out_cubic, ease_out_elastic,
};

// === Overlay + Text Writer ===================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AnimationType {
    #[default]
    Typewriter,     // Original: character by character reveal
    BounceIn,       // Whole text scales from small with bounce (easeOutBounce)
    StaggeredSlide, // Characters slide in from left with staggered timing
    SimpleFade,     // Whole text fades in with alpha (easeOutCubic)
    ElasticReveal,  // Character-by-character reveal with elastic easing
    CascadeZoom,    // Per-character staggered zoom in/out
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
pub struct TextQueue(pub VecDeque<PushOverlayText>);

#[derive(Resource, Clone)]
pub struct OverlayScript {
    pub lines: Vec<PushOverlayText>,
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
                    animation: AnimationType::SimpleFade,
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
pub struct OverlayText;
#[derive(Component)]
pub struct OverlayBackground;

#[derive(Resource)]
pub struct TextWriterState {
    pub timer: f32,
    pub visible_chars: usize,
    pub phase: Phase,
    pub current: Option<PushOverlayText>,
    pub alpha: f32,
    pub animation_type: AnimationType,
    pub scale: f32,               // For BounceIn
    pub x_offset: f32,            // For StaggeredSlide horizontal movement
    pub y_offset: f32,            // For vertical animations
    pub swing_h: f32,             // Horizontal swing offset (calculated by apply_swing_animation)
    pub swing_v: f32,             // Vertical swing offset (calculated by apply_swing_animation)
    pub bg_swing_h: f32,          // Background horizontal swing offset
    pub bg_swing_v: f32,          // Background vertical swing offset
    pub viewport_scale: f32, // Responsive scale based on window width (calculated by apply_swing_animation)
    pub cascade_scales: Vec<f32>, // Per-character scale buffer for CascadeZoom
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
pub enum Phase {
    Idle,
    Typing,
    Dwell,
    FadeOut,
}

#[derive(Resource, Default)]
pub struct BeatPulse {
    pub energy: f32,
}

// === Font Data ==============================================================

#[derive(Resource)]
pub struct BitmapFont {
    pub image: Handle<Image>,
    pub glyph_map: HashMap<char, UVec2>,
    pub cell_size: UVec2,
    pub letter_spacing: u32,
    pub default_coord: UVec2,
}

impl BitmapFont {
    pub fn new(image: Handle<Image>) -> Self {
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

// === Systems ================================================================

pub fn handle_push_events(mut evr: MessageReader<PushOverlayText>, mut queue: ResMut<TextQueue>) {
    for ev in evr.read() {
        queue.0.push_back(ev.clone());
    }
}

pub fn feed_overlay_script(
    script: Res<OverlayScript>,
    state: Res<TextWriterState>,
    mut queue: ResMut<TextQueue>,
) {
    if state.phase == Phase::Idle && queue.0.is_empty() && !script.lines.is_empty() {
        queue.0.extend(script.lines.iter().cloned());
    }
}

pub fn setup_text_overlay(
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
        bevy::asset::RenderAssetUsages::default(),
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
                    left: Val::Px(-TextLayoutConfig::BACKGROUND_OVERHANG_PX),
                    right: Val::Px(-TextLayoutConfig::BACKGROUND_OVERHANG_PX),
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

pub fn typewriter_update(
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

    if let Some(current) = state.current.as_ref() {
        let text = current.text.clone();
        let cps = current.cps;
        let dwell = current.dwell;
        let fade_out = current.fade_out;
        let pulse_strength = (pulse.energy * VisualEffectsConfig::PULSE_SCALING).clamp(0.0, 1.2);
        let breath = (time.elapsed_secs() * VisualEffectsConfig::BREATH_FREQUENCY).sin()
            * VisualEffectsConfig::BREATH_AMPLITUDE
            + 0.9;

        match state.phase {
            Phase::Typing => {
                state.timer += dt;
                let total = text.chars().count();
                let typing_duration = total as f32 / cps;

                match state.animation_type {
                    AnimationType::Typewriter => {
                        // Original: character by character
                        state.visible_chars = (state.timer * cps).floor() as usize;
                        state.alpha = 1.0;
                    }
                    AnimationType::BounceIn => {
                        // All text at once with DRAMATIC bounce (multiple bounces)
                        state.visible_chars = total;
                        let t = (state.timer / BounceConfig::DURATION).clamp(0.0, 1.0);
                        let bounce_t = ease_out_bounce(t);
                        state.scale = 0.05 + bounce_t * 0.95; // Scales from 0.05 to 1.0 - massive bounce!
                        state.alpha = 1.0;
                    }
                    AnimationType::StaggeredSlide => {
                        // Characters slide in from the side with staggered timing
                        let total_duration = total as f32 * StaggeredSlideConfig::CHAR_DELAY
                            + StaggeredSlideConfig::BASE_DURATION;
                        let t = (state.timer / total_duration).clamp(0.0, 1.0);

                        // Calculate how many characters should be visible
                        state.visible_chars =
                            ((t * total_duration) / StaggeredSlideConfig::CHAR_DELAY) as usize;
                        state.visible_chars = state.visible_chars.min(total);

                        // Horizontal slide effect for all visible characters
                        let slide_t = ease_out_cubic(t);
                        state.x_offset = -StaggeredSlideConfig::SLIDE_DISTANCE_PX * (1.0 - slide_t); // Slide from -100px to 0
                        state.alpha = 1.0;
                    }
                    AnimationType::SimpleFade => {
                        // Whole text fades in with alpha
                        state.visible_chars = total;
                        let t = (state.timer / SimpleFadeConfig::DURATION).clamp(0.0, 1.0);
                        state.alpha = ease_out_cubic(t);
                    }
                    AnimationType::ElasticReveal => {
                        // Characters revealed with elastic easing (typewriter variant)
                        let t = (state.timer / (total as f32 * ElasticRevealConfig::TIME_PER_CHAR))
                            .clamp(0.0, 1.0);
                        state.visible_chars = (ease_out_elastic(t) * total as f32) as usize;
                        state.visible_chars = state.visible_chars.min(total);
                        state.alpha = 1.0;
                    }
                    AnimationType::CascadeZoom => {
                        // Characters scale in sequentially from tiny to oversized
                        if state.cascade_scales.len() != total {
                            state
                                .cascade_scales
                                .resize(total, CascadeZoomConfig::MIN_SCALE);
                        }
                        state.visible_chars = total;
                        let timer = state.timer;
                        let cascade_total = ((total.saturating_sub(1) as f32)
                            * CascadeZoomConfig::CHAR_DELAY
                            + CascadeZoomConfig::IN_DURATION)
                            .max(0.0001);
                        let global_t = (timer / cascade_total).clamp(0.0, 1.0);
                        let global_bounce = ease_out_bounce(global_t);
                        let global_overshoot =
                            CascadeZoomConfig::OVERSHOOT * (1.0 - global_t) * global_bounce;
                        state.scale = (CascadeZoomConfig::MIN_SCALE
                            + global_bounce
                                * (CascadeZoomConfig::TARGET_SCALE - CascadeZoomConfig::MIN_SCALE)
                            + global_overshoot)
                            .clamp(CascadeZoomConfig::MIN_SCALE, CascadeZoomConfig::MAX_SCALE);
                        state.alpha = 1.0;
                        for (i, scale) in state.cascade_scales.iter_mut().enumerate() {
                            let start_time = i as f32 * CascadeZoomConfig::CHAR_DELAY;
                            let local_t = ((timer - start_time) / CascadeZoomConfig::IN_DURATION)
                                .clamp(0.0, 1.0);
                            if local_t <= 0.0 {
                                *scale = CascadeZoomConfig::MIN_SCALE;
                                continue;
                            }
                            let bounce = ease_out_bounce(local_t);
                            let overshoot = CascadeZoomConfig::OVERSHOOT * (1.0 - local_t) * bounce;
                            let raw_scale = CascadeZoomConfig::MIN_SCALE
                                + bounce
                                    * (CascadeZoomConfig::TARGET_SCALE
                                        - CascadeZoomConfig::MIN_SCALE)
                                + overshoot;
                            *scale = raw_scale
                                .clamp(CascadeZoomConfig::MIN_SCALE, CascadeZoomConfig::MAX_SCALE);
                        }
                    }
                }

                // Check if animation is complete
                if state.timer
                    >= match state.animation_type {
                        AnimationType::Typewriter => typing_duration,
                        AnimationType::BounceIn => BounceConfig::DURATION,
                        AnimationType::StaggeredSlide => {
                            total as f32 * StaggeredSlideConfig::CHAR_DELAY
                                + StaggeredSlideConfig::BASE_DURATION
                        }
                        AnimationType::SimpleFade => SimpleFadeConfig::DURATION,
                        AnimationType::ElasticReveal => {
                            total as f32 * ElasticRevealConfig::TIME_PER_CHAR
                        }
                        AnimationType::CascadeZoom => {
                            (total.saturating_sub(1) as f32) * CascadeZoomConfig::CHAR_DELAY
                                + CascadeZoomConfig::IN_DURATION
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
                            *scale = CascadeZoomConfig::TARGET_SCALE;
                        }
                    }
                }
            }
            Phase::Dwell => {
                state.timer += dt;
                if state.timer >= dwell {
                    state.phase = Phase::FadeOut;
                    state.timer = 0.0;
                }
                state.alpha = 1.0;
            }
            Phase::FadeOut => {
                state.timer += dt;
                let total_chars = text.chars().count();
                if state.animation_type == AnimationType::CascadeZoom
                    && state.cascade_scales.len() != total_chars
                {
                    state
                        .cascade_scales
                        .resize(total_chars, CascadeZoomConfig::TARGET_SCALE);
                }
                let fade_total = match state.animation_type {
                    AnimationType::CascadeZoom => {
                        let cascade_total = (state.cascade_scales.len().saturating_sub(1) as f32)
                            * CascadeZoomConfig::CHAR_DELAY
                            + CascadeZoomConfig::OUT_DURATION;
                        fade_out.max(cascade_total)
                    }
                    _ => fade_out,
                }
                .max(0.0001);
                let t = (state.timer / fade_total).clamp(0.0, 1.0);

                // Apply fade-out animation based on type
                match state.animation_type {
                    AnimationType::CascadeZoom => {
                        let timer = state.timer;
                        for (i, scale) in state.cascade_scales.iter_mut().enumerate() {
                            let start_time = i as f32 * CascadeZoomConfig::CHAR_DELAY;
                            let local_t = ((timer - start_time) / CascadeZoomConfig::OUT_DURATION)
                                .clamp(0.0, 1.0);
                            if local_t <= 0.0 {
                                continue;
                            }
                            let eased = ease_out_cubic(local_t);
                            let raw = CascadeZoomConfig::TARGET_SCALE * (1.0 - eased);
                            *scale = raw.clamp(0.0, CascadeZoomConfig::TARGET_SCALE);
                        }
                        state.alpha = 1.0 - t;
                    }
                    _ => {
                        // Default: just fade alpha
                        state.alpha = 1.0 - t;
                    }
                }

                if t >= 1.0 {
                    queue.0.pop_front();
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
            text.clone()
        } else {
            text.chars().take(state.visible_chars).collect()
        };
        if let Ok((mut image, mut layout)) = main_query.single_mut() {
            let (glyph_size, texture_size) = if use_cascade {
                let metrics = rebuild_bitmap_text_cascade(
                    &visible,
                    &font,
                    &image.image,
                    &mut images,
                    &state.cascade_scales,
                    CascadeZoomConfig::TARGET_SCALE,
                    CascadeZoomConfig::MAX_SCALE,
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
                * TextLayoutConfig::BASE_SCALE
                * state.viewport_scale)
                .max(0.0);
            let height = (glyph_size.y as f32
                * height_correction
                * zoom
                * scale_factor
                * TextLayoutConfig::BASE_SCALE
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

pub fn start_next_message(state: &mut TextWriterState, queue: &mut TextQueue) {
    if let Some(msg) = queue.0.front().cloned() {
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
                state.x_offset = -StaggeredSlideConfig::SLIDE_DISTANCE_PX; // Start off-screen to the left
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
                state
                    .cascade_scales
                    .resize(total, CascadeZoomConfig::MIN_SCALE);
                state.alpha = 1.0;
                state.scale = CascadeZoomConfig::MIN_SCALE;
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

pub fn apply_swing_animation(
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
            let scale_factor = window.resolution.scale_factor();
            let physical_width = logical_width * scale_factor;
            (physical_width / TextLayoutConfig::DESIGN_WIDTH).clamp(
                TextLayoutConfig::MIN_VIEWPORT_SCALE,
                TextLayoutConfig::MAX_VIEWPORT_SCALE,
            )
        })
        .unwrap_or(1.0);

    // Calculate swing amplitudes responsive to window width
    let swing_h_amplitude = SwingConfig::AMPLITUDE_H_PX * viewport_scale;
    let swing_v_amplitude = SwingConfig::AMPLITUDE_V_PX * viewport_scale;

    // Calculate viewport scale for responsive text sizing
    // Scales between 0.8x and 1.5x based on window width relative to design width
    state.viewport_scale = viewport_scale;

    // Text: base phase - creates elliptical motion using sin/cos 90° apart
    let text_phase = elapsed * SwingConfig::FREQUENCY;
    state.swing_h = text_phase.sin() * swing_h_amplitude;
    state.swing_v = text_phase.cos() * swing_v_amplitude;

    // Background block: offset phase for visually distinct motion
    let bg_phase = text_phase + SwingConfig::PHASE_OFFSET;
    state.bg_swing_h = bg_phase.sin() * swing_h_amplitude;
    state.bg_swing_v = bg_phase.cos() * swing_v_amplitude;
}

pub fn apply_background_swing(
    state: Res<TextWriterState>,
    mut bg_query: Query<&mut Node, With<OverlayBackground>>,
) {
    for mut layout in bg_query.iter_mut() {
        layout.margin.left = Val::Px(state.bg_swing_h);
        layout.margin.top = Val::Px(state.bg_swing_v);
    }
}

// === Bitmap Text Helpers =====================================================

pub fn prepare_font_image(font: &BitmapFont, images: &mut Assets<Image>) {
    if let Some(font_image) = images.get_mut(&font.image)
        && font_image.texture_descriptor.format != TextureFormat::Rgba8UnormSrgb
        && let Some(converted) = font_image.convert(TextureFormat::Rgba8UnormSrgb)
    {
        *font_image = converted;
    }
}

pub fn get_font_data(font: &BitmapFont, images: &Assets<Image>) -> Option<(u32, u32, Vec<u8>)> {
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

pub fn rebuild_bitmap_text(
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
    UVec2::new(width, height)
}

pub struct CascadeImageMetrics {
    pub rest: UVec2,
    pub texture: UVec2,
}

pub fn rebuild_bitmap_text_cascade(
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
    let rest_scale = rest_scale.max(CascadeZoomConfig::MIN_SCALE);
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
            .unwrap_or(CascadeZoomConfig::MIN_SCALE)
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
        let dest_x = dest_x_base + dest_x_offset;
        let dest_y_offset =
            ((max_char_h as i32 - char_h as i32) / 2).clamp(0, max_char_h as i32) as u32;

        let src_x = (coord.x * cell_w) as usize;
        let src_y = (coord.y * cell_h) as usize;
        for y in 0..char_h as usize {
            let src_row = src_y + y.min(cell_h as usize - 1);
            for x in 0..char_w as usize {
                let src_col = src_x + x.min(cell_w as usize - 1);
                let src_index = (src_row * row_span + src_col) * pixel_stride;
                let dst_index =
                    (((y as u32 + dest_y_offset) * width + dest_x + x as u32) * 4) as usize;

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

    CascadeImageMetrics {
        rest: UVec2::new(
            glyphs.len() as u32 * (rest_char_w + spacing) - spacing,
            rest_char_h,
        ),
        texture: UVec2::new(width, height),
    }
}

pub fn spawn_spectrum_bars(commands: &mut Commands) {
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
