//! CRT post-processing effect

use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, Extent3d, ShaderType};
use bevy::shader::ShaderRef;
use bevy::sprite_render::Material2d;

use super::components::CrtQuad;
use super::resources::{CrtMaterialHandle, CrtState, SceneRenderTarget, ScreenSize};

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
    pub crt_enabled: u32,
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

pub fn crt_toggle(kb: Res<ButtonInput<KeyCode>>, mut crt: ResMut<CrtState>) {
    if kb.just_pressed(KeyCode::KeyC) {
        crt.enabled = !crt.enabled;
    }
}
