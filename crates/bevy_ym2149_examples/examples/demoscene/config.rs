use bevy::math::UVec2;

// === Easing Functions (Demoscene Style) =====================================
pub fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t) * (1.0 - t) * (1.0 - t)
}

pub fn ease_out_bounce(t: f32) -> f32 {
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

pub fn ease_out_elastic(t: f32) -> f32 {
    let c5 = (2.0 * std::f32::consts::PI) / 4.5;
    if t == 0.0 {
        0.0
    } else if t == 1.0 {
        1.0
    } else {
        (2.0_f32).powf(-10.0 * t) * ((t - 0.075) * c5).sin() + 1.0
    }
}

// === Animation/Visual Configuration ========================================
pub struct TextLayoutConfig;
impl TextLayoutConfig {
    pub const BASE_SCALE: f32 = 1.5;
    pub const DESIGN_WIDTH: f32 = 1280.0;
    pub const MIN_VIEWPORT_SCALE: f32 = 0.8;
    pub const MAX_VIEWPORT_SCALE: f32 = 1.5;
    pub const BACKGROUND_OVERHANG_PX: f32 = 64.0;
}

pub struct SwingConfig;
impl SwingConfig {
    pub const FREQUENCY: f32 = 2.0;
    pub const AMPLITUDE_H_PX: f32 = 25.0;
    pub const AMPLITUDE_V_PX: f32 = 40.0;
    pub const PHASE_OFFSET: f32 = 0.5;
}

pub struct VisualEffectsConfig;
impl VisualEffectsConfig {
    pub const BREATH_FREQUENCY: f32 = 1.6;
    pub const BREATH_AMPLITUDE: f32 = 0.1;
    pub const PULSE_SCALING: f32 = 0.6;
    pub const STARTUP_FADE_DURATION: f32 = 2.5;
}

pub struct SimpleFadeConfig;
impl SimpleFadeConfig {
    pub const DURATION: f32 = 1.2;
}

pub struct ElasticRevealConfig;
impl ElasticRevealConfig {
    pub const TIME_PER_CHAR: f32 = 0.04;
}

pub struct BounceConfig;
impl BounceConfig {
    pub const DURATION: f32 = 1.3;
}

pub struct StaggeredSlideConfig;
impl StaggeredSlideConfig {
    pub const CHAR_DELAY: f32 = 0.05;
    pub const BASE_DURATION: f32 = 0.5;
    pub const SLIDE_DISTANCE_PX: f32 = 100.0;
}

pub struct CascadeZoomConfig;
impl CascadeZoomConfig {
    pub const IN_DURATION: f32 = 0.55;
    pub const OUT_DURATION: f32 = 0.45;
    pub const CHAR_DELAY: f32 = 0.05;
    pub const MIN_SCALE: f32 = 0.05;
    pub const TARGET_SCALE: f32 = 1.0;
    pub const MAX_SCALE: f32 = 1.25;
    pub const OVERSHOOT: f32 = 0.22;
}

// === Asset / Font Configuration ============================================
pub const BITMAP_CELL_SIZE: UVec2 = UVec2::new(16, 16);
pub const BITMAP_LETTER_SPACING: u32 = 2;
pub const BITMAP_FONT_LAYOUT: [&str; 3] = [
    " !\"#$%&'()*+,-./0123",
    "456789:;<=>?@ABCDEFG",
    "HIJKLMNOPQRSTUVWXYZ[",
];

pub const PLAY_MUSIC: bool = true;
pub const YM_TRACK_PATH: &str = "music/Prelude.ym";
pub const LOGO_TEXTURE_PATH: &str = "textures/vectronix.png";
pub const STAR_TEXTURE_PATH: &str = "textures/star.png";
