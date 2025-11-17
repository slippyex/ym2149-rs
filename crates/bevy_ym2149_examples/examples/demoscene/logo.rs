use bevy::{
    camera::visibility::RenderLayers,
    math::primitives::Rectangle,
    prelude::*,
    render::render_resource::{AsBindGroup, ShaderType},
    shader::ShaderRef,
    sprite_render::{AlphaMode2d, Material2d, MeshMaterial2d},
    window::PrimaryWindow,
};
use bevy_mesh::Mesh2d;

use super::config::{LOGO_TEXTURE_PATH, STAR_TEXTURE_PATH};

pub struct LogoConfig;
impl LogoConfig {
    pub const WIDTH: f32 = 1020.0;
    pub const HEIGHT: f32 = 400.0;
    pub const TOP_MARGIN_PX: f32 = 40.0;
    pub const BOTTOM_MARGIN_PX: f32 = 60.0;
    pub const HOVER_AMPLITUDE_X: f32 = 120.0;
    pub const HOVER_AMPLITUDE_Y: f32 = 0.8; // scaler, actual amplitude computed dynamically
    pub const HOVER_FREQUENCY_X: f32 = 1.6;
    pub const HOVER_FREQUENCY_Y: f32 = 2.2;
    pub const PHASE_DELTA: f32 = std::f32::consts::FRAC_PI_2;
    pub const DISTORT_X_AMPLITUDE: f32 = 0.016;
    pub const DISTORT_X_FREQUENCY: f32 = 5.8;
    pub const DISTORT_X_SPEED: f32 = 1.2;
    pub const DISTORT_Y_AMPLITUDE: f32 = 0.16;
    pub const DISTORT_Y_FREQUENCY: f32 = 10.5;
    pub const DISTORT_Y_SPEED: f32 = 2.9;
}

pub const STAR_COUNT: usize = 12;
pub const STAR_TARGET_SIZE: f32 = 128.0;
pub const STAR_ORBIT_HORIZONTAL_RADIUS: f32 = LogoConfig::WIDTH * 0.55;
pub const STAR_ORBIT_VERTICAL_RADIUS: f32 = LogoConfig::HEIGHT * 0.65;
pub const STAR_ORBIT_SPEED_BASE: f32 = 0.55;
pub const STAR_ROTATION_SPEED_BASE: f32 = 1.6;
pub const STAR_ROTATION_SPEED_VARIATION: f32 = 0.7;
pub const STAR_DEPTH_RANGE: f32 = 25.0;
pub const STAR_ZOOM_AMPLITUDE: f32 = 0.2;
pub const STAR_ZOOM_SPEED_BASE: f32 = 0.9;
pub const STAR_ZOOM_SPEED_VARIATION: f32 = 0.4;

#[derive(AsBindGroup, TypePath, Debug, Clone, Asset)]
pub struct LogoMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub logo_texture: Handle<Image>,
    #[uniform(2)]
    pub params: LogoShaderParams,
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
pub struct LogoShaderParams {
    pub time: f32,
    pub amp_x: f32,
    pub freq_x: f32,
    pub speed_x: f32,
    pub amp_y: f32,
    pub freq_y: f32,
    pub speed_y: f32,
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

#[derive(Component)]
pub struct LogoHover {
    pub phase: Vec2,
}
impl Default for LogoHover {
    fn default() -> Self {
        Self {
            phase: Vec2::new(0.0, LogoConfig::PHASE_DELTA),
        }
    }
}

#[derive(Component)]
pub struct LogoQuad;

#[derive(Component)]
pub struct OrbitingStar {
    pub orbit_offset: f32,
    pub radius: Vec2,
    pub spin_angle: f32,
    pub spin_speed: f32,
    pub zoom_phase: f32,
    pub zoom_speed: f32,
    pub zoom_amplitude: f32,
}

#[derive(Resource)]
pub struct LogoMaterialHandle(pub Handle<LogoMaterial>);

pub fn setup_logo_mesh(
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

pub fn setup_orbiting_stars(mut commands: Commands, asset_server: Res<AssetServer>) {
    let texture: Handle<Image> = asset_server.load(STAR_TEXTURE_PATH);
    for i in 0..STAR_COUNT {
        let t = i as f32 / STAR_COUNT as f32;
        let angle = t * std::f32::consts::TAU;
        let spin_speed = STAR_ROTATION_SPEED_BASE + STAR_ROTATION_SPEED_VARIATION * (0.5 - t);
        let zoom_speed = STAR_ZOOM_SPEED_BASE + STAR_ZOOM_SPEED_VARIATION * (t - 0.5);
        let zoom_phase = angle * 0.73;
        let zoom_amplitude = STAR_ZOOM_AMPLITUDE * (0.7 + 0.6 * (t - 0.5).abs());
        let spin_angle = angle * 0.35;
        let initial_rotation = Quat::from_rotation_z(spin_angle);
        commands.spawn((
            Sprite {
                image: texture.clone(),
                custom_size: Some(Vec2::splat(STAR_TARGET_SIZE)),
                ..default()
            },
            Transform {
                translation: Vec3::new(0.0, 0.0, 40.0),
                rotation: initial_rotation,
                scale: Vec3::ONE,
            },
            OrbitingStar {
                orbit_offset: angle,
                radius: Vec2::new(STAR_ORBIT_HORIZONTAL_RADIUS, STAR_ORBIT_VERTICAL_RADIUS),
                spin_angle,
                spin_speed,
                zoom_phase,
                zoom_speed,
                zoom_amplitude,
            },
            RenderLayers::layer(1),
            Name::new(format!("OrbitingStar{}", i + 1)),
        ));
    }
}

pub fn animate_logo_hover(
    time: Res<Time>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut query: Query<(&mut Transform, &LogoHover)>,
) {
    let elapsed = time.elapsed_secs();
    let (_window_height, half_height) = match windows.single() {
        Ok(window) => {
            let height = window.resolution.height();
            (height, height * 0.5)
        }
        Err(_) => (LogoConfig::HEIGHT, LogoConfig::HEIGHT * 0.5),
    };

    let top_limit = half_height - LogoConfig::TOP_MARGIN_PX - LogoConfig::HEIGHT * 0.5;
    let bottom_limit = -half_height + LogoConfig::BOTTOM_MARGIN_PX + LogoConfig::HEIGHT * 0.5;
    let span = top_limit - bottom_limit;
    let base_center = 0.0;

    for (mut transform, hover) in query.iter_mut() {
        let hover_x = LogoConfig::HOVER_AMPLITUDE_X
            * (elapsed * LogoConfig::HOVER_FREQUENCY_X + hover.phase.x).sin();
        let hover_y = (LogoConfig::HOVER_AMPLITUDE_Y * span * 0.15)
            * (elapsed * LogoConfig::HOVER_FREQUENCY_Y + hover.phase.y).cos();
        transform.translation.x = hover_x;
        let target_y = (base_center + hover_y).clamp(bottom_limit, top_limit);
        transform.translation.y = target_y;
    }
}

pub fn animate_orbiting_stars(
    time: Res<Time>,
    logo_query: Query<&Transform, (With<LogoQuad>, Without<OrbitingStar>)>,
    mut stars: Query<(&mut Transform, &mut OrbitingStar)>,
) {
    let delta = time.delta_secs();
    let elapsed = time.elapsed_secs();
    let (center, base_z) = logo_query
        .iter()
        .next()
        .map(|transform| (transform.translation, transform.translation.z))
        .unwrap_or((Vec3::ZERO, 50.0));
    for (mut transform, mut star) in stars.iter_mut() {
        let orbit_angle =
            (elapsed * STAR_ORBIT_SPEED_BASE + star.orbit_offset).rem_euclid(std::f32::consts::TAU);
        star.spin_angle =
            (star.spin_angle + star.spin_speed * delta).rem_euclid(std::f32::consts::TAU);
        star.zoom_phase =
            (star.zoom_phase + star.zoom_speed * delta).rem_euclid(std::f32::consts::TAU);
        let (sin_angle, cos_angle) = orbit_angle.sin_cos();
        let offset = Vec3::new(
            cos_angle * star.radius.x,
            sin_angle * star.radius.y,
            -sin_angle * STAR_DEPTH_RANGE,
        );
        transform.translation =
            Vec3::new(center.x + offset.x, center.y + offset.y, base_z + offset.z);
        let zoom_scale = 1.0 + star.zoom_amplitude * star.zoom_phase.sin();
        transform.scale = Vec3::splat(zoom_scale.max(0.25));
        transform.rotation = Quat::from_rotation_z(star.spin_angle);
    }
}

pub fn update_logo_material(
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
