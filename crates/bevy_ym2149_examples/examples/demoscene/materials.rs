use bevy::{
    prelude::*,
    render::render_resource::{AsBindGroup, ShaderType},
    shader::ShaderRef,
    sprite_render::Material2d,
    window::PrimaryWindow,
};

#[derive(AsBindGroup, TypePath, Debug, Clone, Asset, Default)]
pub struct CubeFacesMaterial {
    #[uniform(0)]
    pub params: CubeParams,
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
    pub scene_texture: Handle<Image>,
    #[uniform(2)]
    pub params: CrtParams,
}
impl Material2d for CrtPostMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path("shaders/crt_post.wgsl".into())
    }
}

#[derive(ShaderType, Clone, Copy, Debug)]
pub struct CubeParams {
    pub time: f32,
    pub width: f32,
    pub height: f32,
    pub mouse: Vec4, // optional belegt, aktuell 0
    pub frame: u32,
    pub crt_enabled: u32,
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
pub struct CrtParams {
    pub time: f32,
    pub width: f32,
    pub height: f32,
    pub crt_enabled: u32,
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
pub struct MaterialHandle(pub Handle<CubeFacesMaterial>);

#[derive(Resource)]
pub struct CrtMaterialHandle(pub Handle<CrtPostMaterial>);

#[derive(Resource, Clone)]
pub struct SceneRenderTarget(pub Handle<Image>);

#[derive(Resource)]
pub struct CrtState {
    pub enabled: bool,
}

pub fn update_uniforms(
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

    let scale_factor = window.resolution.scale_factor();
    let physical_width = (window.resolution.width() * scale_factor).round().max(1.0);
    let physical_height = (window.resolution.height() * scale_factor).round().max(1.0);

    material.params.time = time.elapsed_secs();
    material.params.width = physical_width;
    material.params.height = physical_height;
    material.params.frame = material.params.frame.wrapping_add(1);
    material.params.crt_enabled = crt_enabled_flag;

    if let Some(crt_mat) = crt_mat
        && let Some(crt_material) = crt_materials.get_mut(&crt_mat.0)
    {
        crt_material.params.time = time.elapsed_secs();
        crt_material.params.width = physical_width;
        crt_material.params.height = physical_height;
        crt_material.params.crt_enabled = crt_enabled_flag;
    }
}
