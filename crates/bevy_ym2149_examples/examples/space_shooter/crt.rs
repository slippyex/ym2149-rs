//! CRT post-processing effect

use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, Extent3d, ShaderType};
use bevy::shader::ShaderRef;
use bevy::sprite_render::Material2d;

use super::components::CrtQuad;
use super::resources::{
    CrtMaterialHandle, CrtState, SceneRenderTarget, ScreenFlash, ScreenShake, ScreenSize,
};

#[derive(AsBindGroup, TypePath, Debug, Clone, Asset)]
pub struct CrtMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub scene_texture: Handle<Image>,
    #[uniform(2)]
    pub params: CrtParams,
}

impl Material2d for CrtMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path("shaders/crt_post.wgsl".into())
    }
}

#[derive(ShaderType, Clone, Copy, Debug, Default)]
pub struct CrtParams {
    pub time: f32,
    pub width: f32,
    pub height: f32,
    /// 0.0 = off, 1.0 = on (kept as float for simpler uniform packing).
    pub crt_enabled: f32,
    /// 0.0..1.0 impact intensity (shake/flash driven).
    pub impact: f32,
    /// Chromatic aberration strength in pixels.
    pub chroma_px: f32,
    /// Grain amount (0.0..1.0 typical range).
    pub grain: f32,
    /// Vignette strength (0.0..1.0).
    pub vignette: f32,
    /// Flash overlay in linear space: rgb = color, a = strength.
    pub flash: Vec4,
}

pub fn sync_render_target(
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

pub fn update_crt_material(
    time: Res<Time>,
    screen: Res<ScreenSize>,
    mut mats: ResMut<Assets<CrtMaterial>>,
    mat_h: Option<Res<CrtMaterialHandle>>,
    crt: Res<CrtState>,
    shake: Res<ScreenShake>,
    flash: Res<ScreenFlash>,
) {
    let Some(h) = mat_h else { return };
    if let Some(mat) = mats.get_mut(&h.0) {
        let trauma = shake.trauma.clamp(0.0, 1.0);
        let flash_rgba = flash.color.to_srgba();
        let flash_strength = (flash.strength() * flash_rgba.alpha).min(0.25);
        let impact = (trauma * 0.8 + flash_strength * 0.6).clamp(0.0, 1.0);

        // Keep defaults subtle; crank them with "impact" for punchy moments.
        let chroma_px = 0.6 + impact * 2.0;
        // Grain easily reads as "too noisy" on crisp modern displays, so keep it very subtle.
        let grain = 0.004 + impact * 0.012;
        let vignette = 0.15 + impact * 0.25;

        mat.params = CrtParams {
            time: time.elapsed_secs(),
            width: screen.width,
            height: screen.height,
            crt_enabled: if crt.enabled { 1.0 } else { 0.0 },
            impact,
            chroma_px,
            grain,
            vignette,
            flash: Vec4::new(
                flash_rgba.red,
                flash_rgba.green,
                flash_rgba.blue,
                flash_strength,
            ),
        };
    }
}

pub fn crt_toggle(kb: Res<ButtonInput<KeyCode>>, mut crt: ResMut<CrtState>) {
    if kb.just_pressed(KeyCode::KeyC) {
        crt.enabled = !crt.enabled;
    }
}
