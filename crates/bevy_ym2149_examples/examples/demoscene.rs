//! Cube Faces Raymarch (ShaderToy Buffer A Port, single pass)
//! Mit Text-Overlay + YM2149 Playback.
//!
//! Shader: `shaders/oldschool.wgsl` (relative to assets directory)

mod demoscene {
    pub mod config;
    pub mod logo;
    pub mod materials;
    pub mod overlay;
}

use bevy::{
    app::AppExit,
    asset::AssetPlugin,
    camera::{ClearColorConfig, RenderTarget, visibility::RenderLayers},
    math::primitives::Rectangle,
    prelude::*,
    render::render_resource::{
        Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
    },
    shader::Shader,
    sprite_render::{Material2dPlugin, MeshMaterial2d},
    ui::{Node, PositionType, Val},
    window::{MonitorSelection, PrimaryWindow, VideoModeSelection, WindowMode, WindowResized},
};
use bevy_mesh::Mesh2d;
use bevy_ym2149::{Ym2149AudioSource, Ym2149Playback, Ym2149Plugin, Ym2149Settings};
use bevy_ym2149_examples::{embedded_asset_plugin, ASSET_BASE};
use bevy_ym2149_viz::Ym2149VizPlugin;

use demoscene::{
    config::{PLAY_MUSIC, VisualEffectsConfig, YM_TRACK_PATH, ease_out_cubic},
    logo::{
        LogoMaterial, animate_logo_hover, animate_orbiting_stars, setup_logo_mesh,
        setup_orbiting_stars, update_logo_material,
    },
    materials::{
        CrtMaterialHandle, CrtPostMaterial, CrtState, CubeFacesMaterial, MaterialHandle,
        SceneRenderTarget, update_uniforms,
    },
    overlay::{
        BeatPulse, OverlayScript, TextQueue, TextWriterState, apply_background_swing,
        apply_swing_animation, feed_overlay_script, handle_push_events, setup_text_overlay,
        spawn_spectrum_bars, typewriter_update,
    },
};

// === Startup Fade ============================================================

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

// === App ====================================================================

fn main() {
    App::new()
        .add_plugins(embedded_asset_plugin())
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
            Ym2149VizPlugin,
        ))
        .add_message::<demoscene::overlay::PushOverlayText>()
        .add_systems(
            Startup,
            (
                setup,
                setup_text_overlay,
                setup_logo_mesh,
                setup_orbiting_stars,
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
                animate_orbiting_stars, // Orbit + spin the star sprites
                update_logo_material, // Update shader uniforms for logo distortion
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
            label: Some("demoscene.offscreen"),
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
        duration: VisualEffectsConfig::STARTUP_FADE_DURATION,
    });

    spawn_spectrum_bars(&mut commands);
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

    let scale_factor = window.resolution.scale_factor();
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
        params: crate::demoscene::materials::CrtParams::default(),
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
        window.mode = match window.mode {
            WindowMode::Windowed => {
                WindowMode::Fullscreen(MonitorSelection::Current, VideoModeSelection::Current)
            }
            _ => WindowMode::Windowed,
        };
    }
}

fn toggle_crt(keys: Res<ButtonInput<KeyCode>>, mut crt: ResMut<CrtState>) {
    if keys.just_pressed(KeyCode::KeyC) {
        crt.enabled = !crt.enabled;
    }
}

fn update_surface_scale_on_resize(
    mut reader: MessageReader<WindowResized>,
    pending: Option<ResMut<PendingSurface>>,
    mut query: Query<&mut Transform, With<SurfaceQuad>>,
) {
    let Some(mut pending) = pending else {
        return;
    };
    for ev in reader.read() {
        let scale = Vec3::new(ev.width * 0.5, ev.height * 0.5, 1.0);
        pending.scale = scale;
        if pending.spawned {
            for mut transform in query.iter_mut() {
                transform.scale = scale;
            }
        }
    }
}

fn exit_on_escape(keys: Res<ButtonInput<KeyCode>>, mut exit: MessageWriter<AppExit>) {
    if keys.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
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
