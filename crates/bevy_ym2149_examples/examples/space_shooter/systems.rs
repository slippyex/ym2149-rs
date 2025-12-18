//! Gameplay systems

use bevy::ecs::query::Or;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_ym2149::Ym2149Playback;
use rand::rngs::SmallRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};

use super::components::*;
use super::config::*;
use super::resources::*;
use super::spawning::*;

// === Type Aliases for Complex Queries ===

type FormationEnemyQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static Transform),
    (
        With<Enemy>,
        Without<DivingEnemy>,
        Without<SpiralEnemy>,
        Without<BossEscort>,
        Without<EnemyEntrance>,
    ),
>;
type AllBulletsQuery<'w, 's> = Query<'w, 's, Entity, Or<(With<PlayerBullet>, With<EnemyBullet>)>>;
type GameplayCleanupQuery<'w, 's> = Query<
    'w,
    's,
    Entity,
    Or<(
        With<FadingText>,
        With<LifeIcon>,
        With<DigitSprite>,
        With<WaveDigit>,
        With<EnergyBarBg>,
        With<EnergyBarFill>,
        With<BossBarBg>,
        With<BossBarFill>,
        With<PowerUp>,
        With<SideBooster>,
    )>,
>;

// === SystemParam Bundles ===

/// Resources needed for spawning game entities
#[derive(SystemParam)]
pub struct SpawnContext<'w> {
    pub sprites: Res<'w, SpriteAssets>,
    pub fonts: Res<'w, FontAssets>,
    pub screen: Res<'w, ScreenSize>,
}

/// Bullet entity queries bundled together
#[derive(SystemParam)]
pub struct BulletQueries<'w, 's> {
    pub player_bullets: Query<'w, 's, Entity, With<PlayerBullet>>,
    pub enemy_bullets: Query<'w, 's, Entity, With<EnemyBullet>>,
}

impl BulletQueries<'_, '_> {
    pub fn all(&self) -> impl Iterator<Item = Entity> + '_ {
        self.player_bullets.iter().chain(self.enemy_bullets.iter())
    }
}

/// Collision event writers bundled together
#[derive(SystemParam)]
pub struct CollisionEvents<'w> {
    pub enemy_killed: MessageWriter<'w, EnemyKilledMsg>,
    pub player_hit: MessageWriter<'w, PlayerHitMsg>,
}

/// Game state mutation resources
#[derive(SystemParam)]
pub struct GameStateMut<'w> {
    pub data: ResMut<'w, GameData>,
    pub powerups: ResMut<'w, PowerUpState>,
    pub next_state: ResMut<'w, NextState<GameState>>,
}

/// Shooting context (sprites + powerups for projectile selection)
#[derive(SystemParam)]
pub struct ShootingContext<'w> {
    pub sprites: Res<'w, SpriteAssets>,
    pub powerups: Res<'w, PowerUpState>,
}

/// Entities to clean up when game ends
#[derive(SystemParam)]
pub struct GameEndCleanup<'w, 's> {
    pub bullets: BulletQueries<'w, 's>,
    pub floating_powerups: Query<'w, 's, Entity, With<PowerUp>>,
}

impl GameEndCleanup<'_, '_> {
    pub fn all(&self) -> impl Iterator<Item = Entity> + '_ {
        self.bullets.all().chain(self.floating_powerups.iter())
    }
}

/// Player-related entity queries
#[derive(SystemParam)]
pub struct PlayerQueries<'w, 's> {
    pub player: Query<'w, 's, Entity, With<Player>>,
    pub side_boosters: Query<'w, 's, Entity, With<SideBooster>>,
}

/// Collision detection queries bundled together
#[derive(SystemParam)]
pub struct CollisionQueries<'w, 's> {
    pub player_bullets: Query<
        'w,
        's,
        (Entity, &'static Transform, Option<&'static BulletDamage>),
        With<PlayerBullet>,
    >,
    pub enemy_bullets: Query<'w, 's, (Entity, &'static Transform), With<EnemyBullet>>,
    pub enemies: EnemyCollisionQuery<'w, 's>,
    pub player: Query<'w, 's, (Entity, &'static Transform), With<Player>>,
    pub divers: Query<'w, 's, (Entity, &'static Transform), With<DivingEnemy>>,
}

/// Entities to clean up on game restart
#[derive(SystemParam)]
pub struct RestartCleanup<'w, 's> {
    pub enemies: Query<'w, 's, Entity, With<Enemy>>,
    pub bosses: Query<'w, 's, Entity, With<Boss>>,
    pub bullets: AllBulletsQuery<'w, 's>,
    pub player: Query<'w, 's, Entity, With<Player>>,
    pub ui: GameplayCleanupQuery<'w, 's>,
}

impl RestartCleanup<'_, '_> {
    pub fn all(&self) -> impl Iterator<Item = Entity> + '_ {
        self.enemies
            .iter()
            .chain(self.bosses.iter())
            .chain(self.bullets.iter())
            .chain(self.ui.iter())
    }
}

/// Visual effects resources bundled together
#[derive(SystemParam)]
pub struct VfxResources<'w> {
    pub flash: ResMut<'w, ScreenFlash>,
    pub shake: ResMut<'w, ScreenShake>,
}

/// Music-related resources
#[derive(SystemParam)]
pub struct MusicResources<'w> {
    pub fade: ResMut<'w, MusicFade>,
    pub enabled: Res<'w, MusicEnabled>,
}

/// Popup and effect entities to clean up
#[derive(SystemParam)]
pub struct EffectCleanup<'w, 's> {
    pub popups: Query<'w, 's, Entity, With<ScorePopup>>,
    pub bubbles: Query<'w, 's, Entity, With<ShieldBubble>>,
}

/// Resources for initializing playing state
#[derive(SystemParam)]
pub struct PlayingStateInit<'w> {
    pub data: ResMut<'w, GameData>,
    pub powerups: ResMut<'w, PowerUpState>,
    pub music: MusicResources<'w>,
}

/// Collision VFX resources (shake + combo tracking)
#[derive(SystemParam)]
pub struct CollisionVfx<'w> {
    pub shake: ResMut<'w, ScreenShake>,
    pub combo: ResMut<'w, ComboTracker>,
    pub fonts: Res<'w, FontAssets>,
    pub flash: ResMut<'w, ScreenFlash>,
}

/// Power-up collection context
#[derive(SystemParam)]
pub struct PowerUpContext<'w, 's> {
    pub state: ResMut<'w, PowerUpState>,
    pub energy: ResMut<'w, PlayerEnergy>,
    pub flash: ResMut<'w, ScreenFlash>,
    pub sprites: Res<'w, SpriteAssets>,
    pub player: Query<'w, 's, Entity, With<Player>>,
    pub side_boosters: Query<'w, 's, Entity, With<SideBooster>>,
}

/// Cleanup context for state transitions (name entry, game over)
#[derive(SystemParam)]
pub struct TransitionCleanup<'w, 's> {
    pub powerups: ResMut<'w, PowerUpState>,
    pub enemies: Query<'w, 's, (Entity, &'static Transform), With<Enemy>>,
    pub bosses: Query<'w, 's, Entity, With<Boss>>,
    pub boss_ui: BossUiQuery<'w, 's>,
    pub game_entities: GameEndCleanup<'w, 's>,
    pub effects: EffectCleanup<'w, 's>,
}

/// Player death handling context
#[derive(SystemParam)]
pub struct PlayerDeathContext<'w> {
    pub scores: Res<'w, HighScoreList>,
    pub respawn_timer: ResMut<'w, PlayerRespawnTimer>,
    pub sfx: MessageWriter<'w, PlaySfxMsg>,
}

// === Type Aliases for Complex Queries ===

type ShieldBubbleQuery<'w, 's> = Query<
    'w,
    's,
    (&'static mut Transform, &'static mut Sprite),
    (With<ShieldBubble>, Without<Player>),
>;

type BossUiQuery<'w, 's> = Query<'w, 's, Entity, Or<(With<BossBarBg>, With<BossBarFill>)>>;

type EnemyCollisionQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static Sprite,
        &'static Enemy,
        Option<&'static BossEscort>,
        &'static mut EnemyHp,
    ),
    With<Enemy>,
>;

type EnemyBulletMovementQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut Transform,
        &'static Sprite,
        &'static mut TrailEmitter,
        Option<&'static BulletVelocity>,
    ),
    (With<EnemyBullet>, Without<PlayerBullet>),
>;

type FormationMovementQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Transform,
    (
        With<Enemy>,
        Without<DivingEnemy>,
        Without<SpiralEnemy>,
        Without<BossEscort>,
        Without<EnemyEntrance>,
    ),
>;

type ParallaxStarQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Transform,
        &'static mut Sprite,
        &'static Star,
        &'static StarLayer,
        &'static ParallaxAnchor,
        &'static StarTwinkle,
    ),
    Without<Player>,
>;

type SideBoosterQuery<'w, 's> = Query<
    'w,
    's,
    (&'static mut Transform, &'static mut Sprite),
    (With<SideBooster>, Without<Booster>),
>;

type PlayerMovementQuery<'w, 's> = Query<
    'w,
    's,
    (&'static mut Transform, &'static mut Sprite),
    (With<Player>, Without<WaveFlyout>),
>;

// === Helper Functions ===

/// Cleans up gameplay entities when game ends (bullets, floating power-ups)
fn cleanup_game_entities(
    cmd: &mut Commands,
    powerups: &mut PowerUpState,
    bullets: impl Iterator<Item = Entity>,
    floating_powerups: impl Iterator<Item = Entity>,
) {
    powerups.reset();
    for e in bullets.chain(floating_powerups) {
        cmd.entity(e).try_despawn();
    }
}

/// Converts enemies to sine-wave floating animation for game over / name entry screens
fn setup_enemies_sine_wave(
    cmd: &mut Commands,
    enemies: &[(Entity, &Transform)],
    rng: &mut SmallRng,
) {
    let mut enemies: Vec<_> = enemies.to_vec();
    enemies.shuffle(rng);

    for (i, (entity, t)) in enemies.into_iter().enumerate() {
        cmd.entity(entity)
            .remove::<DivingEnemy>()
            .insert(GameOverEnemy {
                phase: rng.random_range(0.0..std::f32::consts::TAU),
                amplitude: rng.random_range(80.0..180.0),
                frequency: rng.random_range(0.5..1.5),
                base_pos: t.translation.truncate(),
                delay: i as f32 * 0.08,
                started: false,
            });
    }
}

#[derive(Clone, Copy)]
pub(crate) struct EnemyColumnPos {
    col: i32,
    x: f32,
    y: f32,
}

// === Screen Size ===

pub fn update_screen_size(
    window: Single<&Window, With<PrimaryWindow>>,
    mut screen: ResMut<ScreenSize>,
) {
    if screen.width != window.width() || screen.height != window.height() {
        *screen = ScreenSize::from_window(&window);
    }
}

// === Title Screen ===

pub fn title_input(
    kb: Res<ButtonInput<KeyCode>>,
    mut ns: ResMut<NextState<GameState>>,
    mut exit: MessageWriter<AppExit>,
) {
    if kb.just_pressed(KeyCode::Enter) {
        ns.set(GameState::Playing);
    }
    if kb.just_pressed(KeyCode::KeyH) {
        ns.set(GameState::HighScores);
    }
    if kb.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
}

pub fn title_anim(
    time: Res<Time>,
    fade: Res<ScreenFade>,
    mut title_q: Query<(&mut TextColor, &UiMarker), Without<UiShadow>>,
) {
    let t = time.elapsed_secs();
    let alpha = fade.alpha;

    for (mut color, marker) in title_q.iter_mut() {
        match marker {
            UiMarker::Title => {
                // Pulsing brightness + hue shift
                let pulse = (t * 1.5).sin() * 0.15 + 1.0;
                let hue_shift = (t * 0.5).sin() * 0.1;
                color.0 = Color::srgba(
                    (0.2 + hue_shift) * pulse,
                    0.8 * pulse,
                    (1.0 - hue_shift) * pulse,
                    alpha,
                );
            }
            UiMarker::PressEnter => {
                // Blinking
                let blink = (t * 3.0).sin() * 0.4 + 0.6;
                color.0 = Color::srgba(1.0, 1.0, 0.2, alpha * blink);
            }
            _ => {}
        }
    }
}

pub fn title_subtitle_anim(
    time: Res<Time>,
    fade: Res<ScreenFade>,
    mut q: Query<(&mut TextColor, &UiMarker, Option<&UiShadow>)>,
) {
    let t = time.elapsed_secs();
    let alpha = fade.alpha;

    for (mut color, marker, shadow) in q.iter_mut() {
        match marker {
            UiMarker::Title => {
                if shadow.is_some() {
                    color.0 = Color::srgba(0.0, 0.0, 0.0, alpha * 0.65);
                }
            }
            UiMarker::PressEnter => {
                if shadow.is_some() {
                    let blink = (t * 3.0).sin() * 0.4 + 0.6;
                    color.0 = Color::srgba(0.0, 0.0, 0.0, alpha * 0.55);
                    // Keep the shadow in sync with the blink so the text reads cleanly.
                    color.0 = color.0.with_alpha(color.0.to_srgba().alpha * blink);
                }
            }
            UiMarker::Subtitle => {
                if shadow.is_some() {
                    color.0 = Color::srgba(0.0, 0.0, 0.0, alpha * 0.72);
                } else {
                    // Bright, readable controls line with a small shimmer.
                    let glow = (t * 4.2).sin() * 0.10 + 0.90;
                    color.0 = Color::srgba(0.78 * glow, 0.92 * glow, 1.0 * glow, alpha * 0.95);
                }
            }
            _ => {}
        }
    }
}

fn spawn_title_scene(cmd: &mut Commands, sprites: &SpriteAssets, screen: &ScreenSize) {
    // Big boss silhouette behind the logo.
    cmd.spawn((
        Sprite {
            image: sprites.boss_texture.clone(),
            texture_atlas: Some(TextureAtlas {
                layout: sprites.boss_layout.clone(),
                index: 0,
            }),
            color: Color::srgba(0.55, 0.45, 1.0, 0.22),
            ..default()
        },
        Transform::from_xyz(0.0, screen.half_height * 0.12, 0.20)
            .with_scale(Vec3::splat(BOSS_SCALE * 1.75)),
        TitleDecor {
            base: Vec3::new(0.0, screen.half_height * 0.12, 0.20),
            amp: Vec2::new(42.0, 18.0),
            speed: Vec2::new(0.55, 0.8),
            rot_speed: 0.12,
            scale_base: BOSS_SCALE * 1.75,
            scale_pulse: 0.06,
            alpha: 0.22,
        },
        TitleSceneEntity,
    ));

    // Glow ring behind the boss.
    cmd.spawn((
        Sprite {
            image: sprites.ring_texture.clone(),
            color: Color::srgba(0.35, 0.95, 1.0, 0.10),
            ..default()
        },
        Transform::from_xyz(0.0, screen.half_height * 0.12, 0.18).with_scale(Vec3::splat(14.0)),
        TitleDecor {
            base: Vec3::new(0.0, screen.half_height * 0.12, 0.18),
            amp: Vec2::new(10.0, 6.0),
            speed: Vec2::new(0.35, 0.45),
            rot_speed: -0.18,
            scale_base: 14.0,
            scale_pulse: 0.10,
            alpha: 0.10,
        },
        TitleSceneEntity,
    ));

    // Enemy "swarm" flyby: several ships in a sinus-spiral pattern.
    let swarm_center_y = screen.half_height * 0.12;
    let swarm = [
        (
            sprites.enemy_lips_texture.clone(),
            sprites.enemy_lips_layout.clone(),
            4usize,
            Color::srgba(1.0, 0.55, 0.75, 0.90),
        ),
        (
            sprites.enemy_bonbon_texture.clone(),
            sprites.enemy_bonbon_layout.clone(),
            3usize,
            Color::srgba(1.0, 0.95, 0.55, 0.86),
        ),
        (
            sprites.enemy_alan_texture.clone(),
            sprites.enemy_alan_layout.clone(),
            5usize,
            Color::srgba(0.65, 0.95, 1.0, 0.85),
        ),
    ];
    for i in 0..7 {
        let (tex, layout, last, tint) = swarm[i % swarm.len()].clone();
        let scale = SPRITE_SCALE * (0.62 + (i as f32) * 0.03);
        let z = 0.46 + (i as f32) * 0.003;
        cmd.spawn((
            Sprite {
                image: tex,
                texture_atlas: Some(TextureAtlas { layout, index: 0 }),
                color: tint,
                ..default()
            },
            Transform::from_xyz(0.0, swarm_center_y, z).with_scale(Vec3::splat(scale)),
            Enemy { points: 0 },
            EnemyHp { current: 1 },
            AnimationIndices { first: 0, last },
            AnimationTimer(Timer::from_seconds(1.0 / ANIM_FPS, TimerMode::Repeating)),
            TitleSpiralFlyby {
                dir: if i % 2 == 0 { 1.0 } else { -1.0 },
                progress: 0.0,
                duration: 7.0 + (i as f32) * 0.35,
                wait: 0.9 + (i as f32) * 0.22,
                wait_after: 1.6 + (i as f32) * 0.18,
                base_y: swarm_center_y + (i as f32 - 3.0) * 10.0,
                z,
                scale,
                alpha: tint.to_srgba().alpha,
                radius: 34.0 + (i as f32) * 9.0,
                turns: 1.15 + (i as f32) * 0.15,
                drift_amp: 14.0 + (i as f32) * 2.2,
                drift_freq: 1.1 + (i as f32) * 0.12,
                phase: i as f32 * 0.85,
            },
            TitleSceneEntity,
        ));
    }

    // Player flyby along the bottom.
    let player_id = cmd
        .spawn((
            Sprite::from_atlas_image(
                sprites.player_texture.clone(),
                TextureAtlas {
                    layout: sprites.player_layout.clone(),
                    index: 1,
                },
            ),
            Transform::from_xyz(0.0, -screen.half_height * 0.32, 0.55)
                .with_scale(Vec3::splat(SPRITE_SCALE * 0.95)),
            TitleFlyby {
                dir: 1.0,
                progress: 0.0,
                duration: 5.2,
                wait: 0.35,
                wait_after: 1.3,
                y: -screen.half_height * 0.32,
                z: 0.55,
                scale: SPRITE_SCALE * 0.95,
                alpha: 0.95,
                bob_amp: 10.0,
                bob_freq: 1.25,
                roll_amp: 0.14,
                roll_freq: 2.1,
                phase: 0.0,
            },
            TitleSceneEntity,
        ))
        .id();

    // Booster flame as child to make the flyby feel alive (animation is handled by `animate_sprites`).
    cmd.spawn((
        Sprite::from_atlas_image(
            sprites.booster_texture.clone(),
            TextureAtlas {
                layout: sprites.booster_layout.clone(),
                index: 0,
            },
        ),
        Transform::from_xyz(0.0, -12.0, -0.1),
        ChildOf(player_id),
        Booster,
        AnimationIndices { first: 0, last: 1 },
        AnimationTimer(Timer::from_seconds(0.08, TimerMode::Repeating)),
        TitleSceneEntity,
    ));
}

pub fn ensure_title_screen_scene(
    mut cmd: Commands,
    sprites: Option<Res<SpriteAssets>>,
    screen: Option<Res<ScreenSize>>,
    existing: Query<(), With<TitleSceneEntity>>,
) {
    if !existing.is_empty() {
        return;
    }
    let (Some(sprites), Some(screen)) = (sprites, screen) else {
        return;
    };
    spawn_title_scene(&mut cmd, sprites.as_ref(), screen.as_ref());
}

pub fn exit_title_screen(mut cmd: Commands, q: Query<Entity, With<TitleSceneEntity>>) {
    for e in q.iter() {
        cmd.entity(e).try_despawn();
    }
}

pub fn title_decor_update(
    time: Res<Time>,
    fade: Res<ScreenFade>,
    mut decor: Query<(&mut Transform, &mut Sprite, &TitleDecor), Without<TitleFlyby>>,
) {
    let t = time.elapsed_secs();
    let dt = time.delta_secs();
    let alpha = fade.alpha;

    for (mut tr, mut sprite, d) in decor.iter_mut() {
        let x = (t * d.speed.x).sin() * d.amp.x;
        let y = (t * d.speed.y).cos() * d.amp.y;
        tr.translation = d.base + Vec3::new(x, y, 0.0);
        tr.rotate_z(d.rot_speed * dt);
        let pulse = (t * 1.7).sin() * 0.5 + 0.5;
        let s = d.scale_base * (1.0 + d.scale_pulse * pulse);
        tr.scale = Vec3::splat(s);

        let base = sprite.color.to_srgba();
        sprite.color = Color::srgba(base.red, base.green, base.blue, d.alpha * alpha);
    }
}

#[allow(clippy::type_complexity)]
pub fn title_flyby_update(
    time: Res<Time>,
    fade: Res<ScreenFade>,
    screen: Res<ScreenSize>,
    mut set: ParamSet<(
        Query<
            (&mut Transform, &mut Sprite, &mut TitleFlyby),
            (Without<TitleDecor>, Without<TitleSpiralFlyby>),
        >,
        Query<
            (&mut Transform, &mut Sprite, &mut TitleSpiralFlyby),
            (Without<TitleDecor>, Without<TitleFlyby>),
        >,
    )>,
) {
    let t = time.elapsed_secs();
    let dt = time.delta_secs();
    let alpha = fade.alpha;

    for (mut tr, mut sprite, mut f) in set.p0().iter_mut() {
        if f.wait > 0.0 {
            f.wait = (f.wait - dt).max(0.0);
            let base = sprite.color.to_srgba();
            sprite.color = Color::srgba(base.red, base.green, base.blue, 0.0);
            continue;
        }

        let from_x = -screen.half_width - 140.0;
        let to_x = screen.half_width + 140.0;

        f.progress += dt / f.duration.max(0.001);
        if f.progress >= 1.0 {
            f.progress = 0.0;
            f.wait = f.wait_after;
            f.dir = -f.dir;
        }

        let u = f.progress.clamp(0.0, 1.0);
        let p = u * u * (3.0 - 2.0 * u);
        let x = if f.dir >= 0.0 {
            from_x + (to_x - from_x) * p
        } else {
            to_x + (from_x - to_x) * p
        };

        let bob = (t * f.bob_freq + f.phase).sin() * f.bob_amp;
        let roll = (t * f.roll_freq + f.phase).sin() * f.roll_amp;
        let tilt = if f.dir >= 0.0 { -0.05 } else { 0.05 };

        tr.translation.x = x;
        tr.translation.y = f.y + bob;
        tr.translation.z = f.z;
        tr.rotation = Quat::from_rotation_z(roll + tilt);
        tr.scale = Vec3::splat(f.scale);

        let base = sprite.color.to_srgba();
        sprite.color = Color::srgba(base.red, base.green, base.blue, f.alpha * alpha);
    }

    for (mut tr, mut sprite, mut s) in set.p1().iter_mut() {
        if s.wait > 0.0 {
            s.wait = (s.wait - dt).max(0.0);
            let base = sprite.color.to_srgba();
            sprite.color = Color::srgba(base.red, base.green, base.blue, 0.0);
            continue;
        }

        let from_x = -screen.half_width - 150.0;
        let to_x = screen.half_width + 150.0;

        s.progress += dt / s.duration.max(0.001);
        if s.progress >= 1.0 {
            s.progress = 0.0;
            s.wait = s.wait_after;
            s.dir = -s.dir;
        }

        let u = s.progress.clamp(0.0, 1.0);
        let p = u * u * (3.0 - 2.0 * u);
        let x = if s.dir >= 0.0 {
            from_x + (to_x - from_x) * p
        } else {
            to_x + (from_x - to_x) * p
        };

        let angle = (u * s.turns * std::f32::consts::TAU + s.phase) * s.dir;
        let drift = (t * s.drift_freq + s.phase).sin() * s.drift_amp;
        let r = s.radius * (0.35 + 0.65 * (1.0 - (u - 0.5).abs() * 2.0).clamp(0.0, 1.0));
        let off_x = angle.cos() * r;
        let off_y = angle.sin() * r * 0.28;

        tr.translation.x = x + off_x;
        tr.translation.y = s.base_y + drift + off_y;
        tr.translation.z = s.z;
        tr.rotation = Quat::from_rotation_z(angle.sin() * 0.10);
        tr.scale = Vec3::splat(s.scale);

        let base = sprite.color.to_srgba();
        sprite.color = Color::srgba(base.red, base.green, base.blue, s.alpha * alpha);
    }
}

pub fn screen_fade_update(time: Res<Time>, mut fade: ResMut<ScreenFade>) {
    fade.timer.tick(time.delta());
    let progress = fade.timer.fraction();

    if fade.fading_in {
        fade.alpha = progress;
        if fade.timer.just_finished() {
            fade.fading_in = false;
            fade.alpha = 1.0;
        }
    } else if fade.fading_out {
        fade.alpha = 1.0 - progress;
        if fade.timer.just_finished() {
            fade.fading_out = false;
            fade.alpha = 0.0;
        }
    }
}

pub fn title_auto_cycle(
    time: Res<Time>,
    attract: Res<AttractMode>,
    mut fade: ResMut<ScreenFade>,
    mut timer: ResMut<ScreenCycleTimer>,
    mut ns: ResMut<NextState<GameState>>,
) {
    if !attract.active {
        return;
    }

    timer.0.tick(time.delta());

    // Start fade-out 0.5s before timer ends
    let remaining = timer.0.remaining_secs();
    if remaining <= ScreenFade::FADE_DURATION && !fade.fading_out && fade.alpha > 0.0 {
        fade.start_fade_out();
    }

    // Transition when timer finishes
    if timer.0.just_finished() {
        ns.set(GameState::HighScores);
    }
}

pub fn start_screen_fade_in(mut fade: ResMut<ScreenFade>) {
    fade.start_fade_in();
}

pub fn reset_attract_mode(mut attract: ResMut<AttractMode>) {
    attract.active = false;
}

pub fn activate_attract_mode(mut attract: ResMut<AttractMode>) {
    attract.active = true;
}

// === State Transitions ===

pub fn hide_title_ui(mut q: Query<(&mut Visibility, &UiMarker)>) {
    for (mut v, m) in q.iter_mut() {
        if matches!(
            m,
            UiMarker::Title | UiMarker::PressEnter | UiMarker::Subtitle
        ) {
            *v = Visibility::Hidden;
        }
    }
}

pub fn show_title_ui(mut q: Query<(&mut Visibility, &UiMarker)>) {
    for (mut v, m) in q.iter_mut() {
        if matches!(
            m,
            UiMarker::Title | UiMarker::PressEnter | UiMarker::Subtitle
        ) {
            *v = Visibility::Visible;
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn enter_playing(
    mut cmd: Commands,
    mut state: PlayingStateInit,
    mut timers: ResMut<GameTimers>,
    spawn: SpawnContext,
    mut uiq: Query<(&mut Visibility, &UiMarker)>,
    old_enemies: Query<Entity, With<GameOverEnemy>>,
    mut flash: ResMut<ScreenFlash>,
    mut energy: ResMut<PlayerEnergy>,
    mut drop_queue: ResMut<PowerUpDropQueue>,
    mut powerup_cd: ResMut<PowerUpSpawnCooldown>,
    mut intermission: ResMut<WaveIntermission>,
) {
    // Clean up any leftover enemies from game over
    for e in old_enemies.iter() {
        cmd.entity(e).try_despawn();
    }

    if state.music.enabled.0 {
        request_subsong(&mut state.music.fade, 2);
    }
    state.data.reset();
    state.powerups.reset();
    energy.reset();
    drop_queue.0.clear();
    powerup_cd.0 = Timer::from_seconds(0.0, TimerMode::Once);
    intermission.cancel();
    timers.spiral = Timer::from_seconds(state.data.spiral_interval(), TimerMode::Once);
    timers.dive = Timer::from_seconds(state.data.dive_interval(), TimerMode::Once);
    timers.enemy_shoot = Timer::from_seconds(state.data.enemy_shoot_rate(), TimerMode::Once);
    timers.shoot = Timer::from_seconds(0.0, TimerMode::Once);
    spawn_player(&mut cmd, spawn.screen.half_height, &spawn.sprites, false);
    spawn_fading_text(&mut cmd, &spawn.fonts, "", 1.2, Color::NONE, true);
    super::ui::spawn_wave_transition_fx(&mut cmd, &spawn.fonts, spawn.screen.as_ref(), 1, 1.2);
    flash.trigger(0.22, Color::srgba(0.35, 0.55, 1.0, 0.25));

    spawn_life_icons(&mut cmd, &spawn.sprites, &spawn.screen, state.data.lives);
    spawn_score_digits(
        &mut cmd,
        &spawn.sprites,
        &spawn.screen,
        ScoreType::Score,
        state.data.score,
    );
    spawn_score_digits(
        &mut cmd,
        &spawn.sprites,
        &spawn.screen,
        ScoreType::HighScore,
        state.data.high_score,
    );
    spawn_wave_digits(&mut cmd, &spawn.sprites, &spawn.screen, state.data.wave);
    spawn_energy_bar(&mut cmd, spawn.screen.as_ref(), energy.as_ref());

    for (mut v, _) in uiq.iter_mut() {
        *v = Visibility::Hidden;
    }
}

pub fn enter_gameover(
    mut cmd: Commands,
    mut music: MusicResources,
    mut uiq: Query<(Entity, &mut Visibility, &UiMarker)>,
    mut cleanup: TransitionCleanup,
    mut intermission: ResMut<WaveIntermission>,
    mut combo: ResMut<ComboTracker>,
    mut rng: Local<Option<SmallRng>>,
) {
    let rng = rng.get_or_insert_with(SmallRng::from_os_rng);
    intermission.cancel();
    if music.enabled.0 {
        request_subsong(&mut music.fade, 3);
    }

    cleanup_game_entities(
        &mut cmd,
        &mut cleanup.powerups,
        cleanup.game_entities.all(),
        std::iter::empty(),
    );

    // Also clean up score popups and shield bubbles
    for e in cleanup.effects.popups.iter() {
        cmd.entity(e).try_despawn();
    }
    for e in cleanup.effects.bubbles.iter() {
        cmd.entity(e).try_despawn();
    }
    for e in cleanup.bosses.iter() {
        cmd.entity(e).try_despawn();
    }
    for e in cleanup.boss_ui.iter() {
        cmd.entity(e).try_despawn();
    }

    cmd.insert_resource(ScreenCycleTimer::default());
    combo.count = 0;
    combo.timer = 0.0;

    for (entity, mut v, m) in uiq.iter_mut() {
        if *m == UiMarker::GameOver {
            *v = Visibility::Visible;
            cmd.entity(entity).insert(GameOverUi { base_top: 45.0 });
        }
        if *m == UiMarker::Combo {
            *v = Visibility::Hidden;
        }
    }

    let enemies: Vec<_> = cleanup.enemies.iter().collect();
    setup_enemies_sine_wave(&mut cmd, &enemies, rng);
}

pub fn gameover_auto_return(
    time: Res<Time>,
    mut timer: ResMut<ScreenCycleTimer>,
    mut ns: ResMut<NextState<GameState>>,
) {
    timer.0.tick(time.delta());
    if timer.0.just_finished() {
        ns.set(GameState::TitleScreen);
    }
}

pub fn exit_gameover(
    mut cmd: Commands,
    mut q: Query<(Entity, &mut Visibility, &UiMarker)>,
    enemies: Query<Entity, With<GameOverEnemy>>,
    explosions: Query<Entity, With<Explosion>>,
    popups: Query<Entity, With<ScorePopup>>,
) {
    for (entity, mut v, m) in q.iter_mut() {
        if *m == UiMarker::GameOver {
            *v = Visibility::Hidden;
            cmd.entity(entity).remove::<GameOverUi>();
        }
    }
    // Despawn all game entities when leaving game over
    for e in enemies.iter() {
        cmd.entity(e).try_despawn();
    }
    for e in explosions.iter() {
        cmd.entity(e).try_despawn();
    }
    for e in popups.iter() {
        cmd.entity(e).try_despawn();
    }
}

// === Message Handlers ===

pub fn handle_sfx_events(
    mut events: MessageReader<PlaySfxMsg>,
    gist: Option<Res<GistRes>>,
    sfx: Option<Res<Sfx>>,
) {
    for ev in events.read() {
        if let (Some(g), Some(s)) = (&gist, &sfx)
            && let Ok(mut p) = g.0.lock()
        {
            p.play_sound(
                match ev.0 {
                    SfxType::Laser => &s.laser,
                    SfxType::Explode => &s.explode,
                    SfxType::Death => &s.death,
                    SfxType::PowerUp => &s.powerup_pickup,
                },
                None,
                None,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn handle_wave_complete(
    mut cmd: Commands,
    mut events: MessageReader<WaveCompleteMsg>,
    gd: Res<GameData>,
    screen: Res<ScreenSize>,
    fonts: Res<FontAssets>,
    mut flash: ResMut<ScreenFlash>,
    mut drop_queue: ResMut<PowerUpDropQueue>,
    mut powerup_cd: ResMut<PowerUpSpawnCooldown>,
    mut intermission: ResMut<WaveIntermission>,
    player_q: Query<(Entity, &Transform), With<Player>>,
) {
    for _ in events.read() {
        // Prevent re-triggering while we are already transitioning.
        if intermission.is_active() {
            continue;
        }

        let next_wave = gd.wave.saturating_add(1);
        drop_queue.0.clear();
        for kind in [
            PowerUpType::Heal,
            PowerUpType::RapidFire,
            PowerUpType::TripleShot,
            PowerUpType::SpeedBoost,
            PowerUpType::PowerShot,
        ] {
            drop_queue.0.push_back(kind);
            drop_queue.0.push_back(kind);
        }

        // "Wave cleared" + short breather before the next wave begins.
        spawn_fading_text(
            &mut cmd,
            &fonts,
            "WAVE  CLEARED",
            1.8,
            Color::srgba(0.35, 1.0, 0.55, 1.0),
            false,
        );

        // Start player flyout animation (accelerate up, wrap to bottom, return)
        let player_y = -screen.half_height + 60.0;
        if let Ok((entity, _)) = player_q.single() {
            cmd.entity(entity).insert(WaveFlyout {
                phase: WaveFlyoutPhase::AccelerateUp,
                velocity: 200.0,
                target_y: player_y,
            });
        }

        intermission.start(next_wave, 3.5); // Slightly longer for flyout
        powerup_cd.0 = Timer::from_seconds(2.0, TimerMode::Once);
        flash.trigger(0.18, Color::srgba(0.25, 0.45, 1.0, 0.12));
    }
}

#[allow(clippy::too_many_arguments)]
pub fn wave_intermission_update(
    mut cmd: Commands,
    time: Res<Time>,
    mut gd: ResMut<GameData>,
    mut timers: ResMut<GameTimers>,
    mut intermission: ResMut<WaveIntermission>,
    sprites: Res<SpriteAssets>,
    fonts: Res<FontAssets>,
    screen: Res<ScreenSize>,
    rotation: Res<WavePatternRotation>,
    mut flash: ResMut<ScreenFlash>,
) {
    if !intermission.is_active() {
        return;
    }

    intermission.timer.tick(time.delta());
    if !intermission.timer.just_finished() {
        return;
    }

    let Some(next_wave) = intermission.pending_wave.take() else {
        return;
    };

    gd.wave = next_wave;
    gd.enemy_direction = 1.0;
    timers.dive = Timer::from_seconds(gd.dive_interval(), TimerMode::Once);
    timers.enemy_shoot = Timer::from_seconds(gd.enemy_shoot_rate(), TimerMode::Once);
    timers.spiral = Timer::from_seconds(gd.spiral_interval(), TimerMode::Once);

    spawn_enemies_for_wave(
        &mut cmd,
        &sprites,
        screen.as_ref(),
        gd.wave,
        rotation.offset,
    );
    super::ui::spawn_wave_transition_fx(&mut cmd, &fonts, screen.as_ref(), gd.wave, 1.2);
    flash.trigger(0.18, Color::srgba(0.35, 0.55, 1.0, 0.14));
}

pub fn handle_player_hit(
    mut cmd: Commands,
    mut events: MessageReader<PlayerHitMsg>,
    mut state: GameStateMut,
    player_q: PlayerQueries,
    mut death: PlayerDeathContext,
    mut vfx: VfxResources,
) {
    for _ in events.read() {
        death.sfx.write(PlaySfxMsg(SfxType::Death));
        state.data.lives = state.data.lives.saturating_sub(1);

        // Reset power-ups and remove side boosters
        state.powerups.reset();
        for e in player_q.side_boosters.iter() {
            cmd.entity(e).try_despawn();
        }

        for e in player_q.player.iter() {
            cmd.entity(e).try_despawn();
        }

        // Explosion is spawned in collisions system at player position

        // Death flash - bright red overlay
        let flash_color = Color::srgba(1.0, 0.0, 0.0, 0.7);
        vfx.flash.trigger(0.6, flash_color);

        // Extra screen shake on death
        vfx.shake.add_trauma(1.0);

        if state.data.lives == 0 {
            if death.scores.is_high_score(state.data.score) {
                state.next_state.set(GameState::NameEntry);
            } else {
                state.next_state.set(GameState::GameOver);
            }
        } else {
            // Start respawn timer instead of immediate respawn
            death.respawn_timer.start();
        }
    }
}

/// System to handle delayed player respawn
pub fn player_respawn_system(
    mut cmd: Commands,
    time: Res<Time>,
    mut respawn_timer: ResMut<PlayerRespawnTimer>,
    sprites: Res<SpriteAssets>,
    screen: Res<ScreenSize>,
    mut flash: ResMut<ScreenFlash>,
    mut energy: ResMut<PlayerEnergy>,
) {
    if respawn_timer.tick(time.delta()) {
        // Timer finished, respawn with invincibility
        spawn_player(&mut cmd, screen.half_height, &sprites, true);
        energy.reset();

        // Dramatic spawn flash - white/cyan
        let flash_color = Color::srgba(0.5, 1.0, 1.0, 0.6);
        flash.trigger(0.4, flash_color);
    }
}

fn award_points(gd: &mut GameData, points: u32, extra: &mut MessageWriter<ExtraLifeMsg>) {
    gd.score = gd.score.saturating_add(points);
    gd.high_score = gd.high_score.max(gd.score);

    if gd.score < gd.next_extra_life {
        return;
    }

    // Award at most one 1UP per scoring event to avoid "boss kill => +5 lives" bursts.
    extra.write(ExtraLifeMsg);

    // Push the next threshold above the current score with a steeper exponential curve.
    // (Energy bar already makes each life last longer, so 1UPs should be rare.)
    let mut next = gd.next_extra_life.max(EXTRA_LIFE_SCORE);
    loop {
        let grown = (next as f32 * 1.90).round() as u32;
        let mut candidate = grown.max(next.saturating_add(5000));
        candidate = candidate.div_ceil(1000) * 1000; // round to 1k
        next = candidate;
        if next > gd.score {
            break;
        }
    }
    gd.next_extra_life = next;
}

pub fn handle_enemy_killed(
    mut events: MessageReader<EnemyKilledMsg>,
    mut gd: ResMut<GameData>,
    mut sfx: MessageWriter<PlaySfxMsg>,
    mut extra: MessageWriter<ExtraLifeMsg>,
) {
    for ev in events.read() {
        award_points(&mut gd, ev.0, &mut extra);
        sfx.write(PlaySfxMsg(SfxType::Explode));
    }
}

pub fn handle_extra_life(
    mut cmd: Commands,
    mut events: MessageReader<ExtraLifeMsg>,
    mut gd: ResMut<GameData>,
    fonts: Res<FontAssets>,
) {
    for _ in events.read() {
        let before = gd.lives;
        gd.lives = (gd.lives + 1).min(MAX_LIVES);
        if gd.lives == before {
            continue;
        }
        spawn_fading_text(
            &mut cmd,
            &fonts,
            "1UP",
            1.5,
            Color::srgba(0.2, 1.0, 0.2, 1.0),
            false,
        );
    }
}

// === Gameplay ===

pub fn fading_text_update(
    mut cmd: Commands,
    time: Res<Time>,
    sprites: Res<SpriteAssets>,
    screen: Res<ScreenSize>,
    gd: Res<GameData>,
    rotation: Res<WavePatternRotation>,
    mut q: Query<(Entity, &mut FadingText, Option<&mut TextColor>)>,
) {
    for (entity, mut ft, color) in q.iter_mut() {
        ft.timer.tick(time.delta());
        if let Some(mut c) = color {
            c.0 = ft.color.with_alpha(1.0 - ft.timer.fraction());
        }
        if ft.timer.is_finished() {
            cmd.entity(entity).try_despawn();
            if ft.spawn_enemies {
                spawn_enemies_for_wave(
                    &mut cmd,
                    &sprites,
                    screen.as_ref(),
                    gd.wave,
                    rotation.offset,
                );
            }
        }
    }
}

pub fn player_movement(
    kb: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut q: PlayerMovementQuery,
    screen: Res<ScreenSize>,
    powerups: Res<PowerUpState>,
) {
    let Ok((mut t, mut sprite)) = q.single_mut() else {
        return;
    };
    let hw = screen.half_width - PLAYER_SIZE.x / 2.0;
    let dir = kb.pressed(KeyCode::ArrowRight) as i32 - kb.pressed(KeyCode::ArrowLeft) as i32;
    let speed = if powerups.speed_boost {
        PLAYER_SPEED * SPEED_BOOST_MULT
    } else {
        PLAYER_SPEED
    };
    t.translation.x = (t.translation.x + dir as f32 * speed * time.delta_secs()).clamp(-hw, hw);

    if let Some(atlas) = &mut sprite.texture_atlas {
        atlas.index = match dir {
            -1 => 0,
            1 => 2,
            _ => 1,
        };
    }
}

/// Animate player flyout during wave transitions
pub fn wave_flyout_system(
    mut cmd: Commands,
    time: Res<Time>,
    screen: Res<ScreenSize>,
    mut q: Query<(Entity, &mut Transform, &mut WaveFlyout), With<Player>>,
) {
    let dt = time.delta_secs();

    for (entity, mut transform, mut flyout) in q.iter_mut() {
        match flyout.phase {
            WaveFlyoutPhase::AccelerateUp => {
                // Accelerate upward
                flyout.velocity += 800.0 * dt;
                flyout.velocity = flyout.velocity.min(1200.0);
                transform.translation.y += flyout.velocity * dt;

                // Center the ship horizontally while flying
                let center_speed = 200.0 * dt;
                if transform.translation.x.abs() > center_speed {
                    transform.translation.x -= transform.translation.x.signum() * center_speed;
                } else {
                    transform.translation.x = 0.0;
                }

                // When off the top of screen, wrap to bottom
                if transform.translation.y > screen.half_height + 50.0 {
                    flyout.phase = WaveFlyoutPhase::WrapToBottom;
                }
            }
            WaveFlyoutPhase::WrapToBottom => {
                // Teleport to bottom of screen
                transform.translation.y = -screen.half_height - 50.0;
                transform.translation.x = 0.0;
                flyout.velocity = 400.0;
                flyout.phase = WaveFlyoutPhase::ReturnToPosition;
            }
            WaveFlyoutPhase::ReturnToPosition => {
                // Decelerate as we approach target
                let dist = flyout.target_y - transform.translation.y;
                if dist > 0.0 {
                    // Still below target, move up
                    let approach_speed = (dist * 3.0).min(flyout.velocity);
                    transform.translation.y += approach_speed * dt;

                    // Slow down as we get close
                    if dist < 100.0 {
                        flyout.velocity = (flyout.velocity - 400.0 * dt).max(100.0);
                    }
                }

                // Reached target position
                if transform.translation.y >= flyout.target_y - 2.0 {
                    transform.translation.y = flyout.target_y;
                    cmd.entity(entity).remove::<WaveFlyout>();
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn player_shooting(
    mut cmd: Commands,
    kb: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut timers: ResMut<GameTimers>,
    pq: Query<&Transform, (With<Player>, Without<WaveFlyout>)>,
    player_bullets: Query<Entity, With<PlayerBullet>>,
    ctx: ShootingContext,
    mut sfx: MessageWriter<PlaySfxMsg>,
) {
    timers.shoot.tick(time.delta());
    // Hold-to-fire is always enabled; RapidFire improves the cooldown/cap.
    let wants_fire = kb.pressed(KeyCode::Space);
    if wants_fire
        && timers.shoot.is_finished()
        && let Ok(pt) = pq.single()
    {
        // Galaxian-style pressure: avoid screen-filling bullet spam.
        let mut cap = 2usize;
        if ctx.powerups.rapid_fire {
            cap += 1;
        }
        if ctx.powerups.triple_shot {
            // Allow at least one triple-salvo to exist on screen; with RapidFire, allow a follow-up.
            cap = cap.max(3);
            if ctx.powerups.rapid_fire {
                cap += 1;
            }
        }
        if player_bullets.iter().count() >= cap {
            return;
        }

        let base_pos = Vec3::new(
            pt.translation.x,
            pt.translation.y + PLAYER_SIZE.y / 2.0,
            1.0,
        );

        // Choose projectile texture based on power-ups
        let (texture, layout, last_frame, scale) = if ctx.powerups.power_shot {
            (
                ctx.sprites.power_shot_texture.clone(),
                ctx.sprites.power_shot_layout.clone(),
                3,
                SPRITE_SCALE * 0.6,
            )
        } else if ctx.powerups.triple_shot {
            (
                ctx.sprites.triple_shot_texture.clone(),
                ctx.sprites.triple_shot_layout.clone(),
                1,
                SPRITE_SCALE * 0.5,
            )
        } else {
            (
                ctx.sprites.player_bullet_texture.clone(),
                ctx.sprites.player_bullet_layout.clone(),
                1,
                SPRITE_SCALE * 0.5,
            )
        };

        // Spawn bullets based on power-ups
        let angles: &[f32] = if ctx.powerups.triple_shot {
            &[-TRIPLE_SHOT_SPREAD, 0.0, TRIPLE_SHOT_SPREAD]
        } else {
            &[0.0]
        };

        for &angle_deg in angles {
            let angle_rad = angle_deg.to_radians();
            let offset_x = angle_rad.sin() * 10.0;
            let pos = base_pos + Vec3::new(offset_x, 0.0, 0.0);
            let dmg = if ctx.powerups.power_shot { 2 } else { 1 };

            cmd.spawn((
                Sprite::from_atlas_image(
                    texture.clone(),
                    TextureAtlas {
                        layout: layout.clone(),
                        index: 0,
                    },
                ),
                Transform::from_translation(pos)
                    .with_scale(Vec3::splat(scale))
                    .with_rotation(Quat::from_rotation_z(-angle_rad)),
                PlayerBullet,
                BulletDamage(dmg),
                TrailEmitter {
                    timer: Timer::from_seconds(0.03, TimerMode::Repeating),
                    alpha: 0.28,
                },
                GameEntity,
                AnimationIndices {
                    first: 0,
                    last: last_frame,
                },
                AnimationTimer(Timer::from_seconds(0.05, TimerMode::Repeating)),
            ));
        }

        sfx.write(PlaySfxMsg(SfxType::Laser));
        let fire_rate = if ctx.powerups.rapid_fire {
            RAPID_FIRE_RATE
        } else {
            BASE_FIRE_RATE
        };
        timers.shoot = Timer::from_seconds(fire_rate, TimerMode::Once);
    }
}

pub fn bullet_movement(
    mut cmd: Commands,
    time: Res<Time>,
    screen: Res<ScreenSize>,
    mut player_bullets: Query<
        (Entity, &mut Transform, &Sprite, &mut TrailEmitter),
        With<PlayerBullet>,
    >,
    mut enemy_bullets: EnemyBulletMovementQuery<'_, '_>,
) {
    let (hw, hh) = (screen.half_width, screen.half_height);
    let dt = time.delta_secs();

    for (e, mut t, sprite, mut trail) in player_bullets.iter_mut() {
        let before = *t;

        // Get direction from rotation (bullets are rotated with -angle_rad)
        let (_, angle) = t.rotation.to_axis_angle();
        let dir = Vec2::new(-angle.sin(), angle.cos());
        t.translation.x += dir.x * BULLET_SPEED * dt;
        t.translation.y += dir.y * BULLET_SPEED * dt;

        trail.timer.tick(time.delta());
        if trail.timer.just_finished() {
            let mut ghost_sprite = sprite.clone();
            ghost_sprite.color = Color::srgba(0.35, 0.95, 1.0, trail.alpha);
            cmd.spawn((
                ghost_sprite,
                Transform {
                    translation: before.translation + Vec3::new(0.0, 0.0, -0.05),
                    rotation: before.rotation,
                    scale: before.scale,
                },
                TrailGhost {
                    timer: Timer::from_seconds(0.12, TimerMode::Once),
                    start_alpha: trail.alpha,
                },
                GameEntity,
            ));
        }

        if t.translation.y > hh + 20.0 || t.translation.x.abs() > hw + 20.0 {
            cmd.entity(e).try_despawn();
        }
    }
    for (e, mut t, sprite, mut trail, vel) in enemy_bullets.iter_mut() {
        let before = *t;

        if let Some(v) = vel {
            t.translation.x += v.0.x * dt;
            t.translation.y += v.0.y * dt;
        } else {
            t.translation.y -= ENEMY_BULLET_SPEED * dt;
        }

        trail.timer.tick(time.delta());
        if trail.timer.just_finished() {
            let mut ghost_sprite = sprite.clone();
            ghost_sprite.color = Color::srgba(1.0, 0.35, 0.35, trail.alpha);
            cmd.spawn((
                ghost_sprite,
                Transform {
                    translation: before.translation + Vec3::new(0.0, 0.0, -0.05),
                    rotation: before.rotation,
                    scale: before.scale,
                },
                TrailGhost {
                    timer: Timer::from_seconds(0.12, TimerMode::Once),
                    start_alpha: trail.alpha,
                },
                GameEntity,
            ));
        }

        if t.translation.y < -hh - 20.0 || t.translation.x.abs() > hw + 20.0 {
            cmd.entity(e).try_despawn();
        }
    }
}

pub fn enemy_formation_movement(
    time: Res<Time>,
    mut gd: ResMut<GameData>,
    mut q: FormationMovementQuery<'_, '_>,
    screen: Res<ScreenSize>,
) {
    let hw = screen.half_width - ENEMY_SIZE.x;
    let remaining = q.iter().count();
    let late_wave_boost = if remaining <= 8 { 1.35 } else { 1.0 };
    let speed = (48.0 + (gd.wave.saturating_sub(1) as f32) * 2.6) * late_wave_boost;
    let drop = (20.0 + (gd.wave.saturating_sub(1) as f32) * 0.7).min(30.0);
    // Never let the formation drift into the player area (player sits at -hh + 60).
    let min_y = (-screen.half_height + 200.0).min(screen.half_height - 120.0);
    let edge = q.iter().any(|t| {
        (t.translation.x > hw && gd.enemy_direction > 0.0)
            || (t.translation.x < -hw && gd.enemy_direction < 0.0)
    });
    if edge {
        gd.enemy_direction *= -1.0;
    }
    for mut t in q.iter_mut() {
        t.translation.x += gd.enemy_direction * speed * time.delta_secs();
        if edge {
            t.translation.y -= drop;
        }
        if t.translation.y < min_y {
            t.translation.y = min_y;
        }
    }
}

/// Animate enemies flying in from off-screen to their formation positions.
pub fn enemy_entrance_system(
    mut cmd: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut Transform, &mut EnemyEntrance), With<Enemy>>,
) {
    let dt = time.delta_secs();

    for (entity, mut transform, mut entrance) in q.iter_mut() {
        // Handle delay before starting animation
        if entrance.delay > 0.0 {
            entrance.delay -= dt;
            continue;
        }

        // Update progress
        entrance.progress += dt / entrance.duration;
        let t = entrance.progress.clamp(0.0, 1.0);

        // Smooth easing (ease-out cubic for deceleration feel)
        let eased = 1.0 - (1.0 - t).powi(3);

        // Calculate base position along the path
        let base_pos = entrance.start.lerp(entrance.target, eased);

        // Add sine wave wobble perpendicular to movement direction
        let sine_offset =
            (t * entrance.sine_freq * std::f32::consts::TAU).sin() * entrance.sine_amp;

        // Calculate perpendicular direction for sine offset
        let dir = (entrance.target - entrance.start).normalize_or_zero();
        let perp = Vec2::new(-dir.y, dir.x);

        // Apply wobble (decreases as we approach target)
        let wobble_factor = 1.0 - eased;
        let final_pos = base_pos + perp * sine_offset * wobble_factor;

        transform.translation.x = final_pos.x;
        transform.translation.y = final_pos.y;

        // Slight rotation based on pattern and wobble for dynamic feel
        let roll = match entrance.pattern {
            EntrancePattern::SweepLeft | EntrancePattern::SweepRight => {
                sine_offset * 0.008 * wobble_factor
            }
            EntrancePattern::FromLeft => -0.15 * wobble_factor,
            EntrancePattern::FromRight => 0.15 * wobble_factor,
            EntrancePattern::FromTop => sine_offset * 0.005 * wobble_factor,
        };
        transform.rotation = Quat::from_rotation_z(roll);

        // Remove component when animation is complete
        if entrance.progress >= 1.0 {
            transform.translation.x = entrance.target.x;
            transform.translation.y = entrance.target.y;
            transform.rotation = Quat::IDENTITY;
            cmd.entity(entity).remove::<EnemyEntrance>();
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn initiate_spirals(
    mut cmd: Commands,
    time: Res<Time>,
    gd: Res<GameData>,
    mut timers: ResMut<GameTimers>,
    eq: FormationEnemyQuery,
    spirals: Query<(), With<SpiralEnemy>>,
    pq: Query<&Transform, With<Player>>,
    mut rng: Local<Option<SmallRng>>,
) {
    if gd.wave < 9 {
        return;
    }
    let rng = rng.get_or_insert_with(SmallRng::from_os_rng);
    timers.spiral.tick(time.delta());
    if !timers.spiral.is_finished() {
        return;
    }

    timers.spiral = Timer::from_seconds(gd.spiral_interval(), TimerMode::Once);
    if spirals.iter().count() >= gd.max_spirals() {
        return;
    }

    let Ok(pt) = pq.single() else { return };
    let player_x = pt.translation.x;

    let candidates: Vec<_> = eq
        .iter()
        // Prefer upper ships for this attack (keeps spacing safe).
        .filter(|(_, t)| t.translation.y > 140.0)
        .collect();
    if candidates.is_empty() {
        return;
    }

    // Send a small "formation" of spirals: 2..4 ships with phase offsets.
    let remaining_slots = gd.max_spirals().saturating_sub(spirals.iter().count());
    let batch = (2 + (gd.wave as usize / 7).min(2))
        .min(remaining_slots)
        .min(candidates.len());

    // Pick a center candidate near the player x, then pick neighbors around it.
    let mut center = candidates[rng.random_range(0..candidates.len())];
    let mut best_score = (center.1.translation.x - player_x).abs();
    for _ in 0..candidates.len().min(10) {
        let cand = candidates[rng.random_range(0..candidates.len())];
        let score = (cand.1.translation.x - player_x).abs() + rng.random::<f32>() * 28.0;
        if score < best_score {
            center = cand;
            best_score = score;
        }
    }

    let mut picks = vec![center];
    while picks.len() < batch {
        let cand = candidates[rng.random_range(0..candidates.len())];
        if picks.iter().any(|(e, _)| *e == cand.0) {
            continue;
        }
        picks.push(cand);
    }

    for (i, (e, t)) in picks.into_iter().enumerate() {
        let curve_dir = if i % 2 == 0 { 1.0 } else { -1.0 };
        let phase = i as f32 * 0.65;
        let turns = 1.35 + rng.random::<f32>() * 0.9;
        let radius = 120.0 + rng.random::<f32>() * 70.0;
        let wobble_amp = 10.0 + rng.random::<f32>() * 18.0;
        let wobble_freq = 1.2 + rng.random::<f32>() * 1.1;

        cmd.entity(e).insert(SpiralEnemy {
            target_x: player_x + rng.random_range(-120.0..120.0),
            returning: false,
            start_y: t.translation.y,
            original: Vec2::new(t.translation.x, t.translation.y),
            progress: phase,
            radius,
            turns,
            curve_dir,
            wobble_amp,
            wobble_freq,
        });
    }
}

pub fn boss_movement(
    time: Res<Time>,
    screen: Res<ScreenSize>,
    mut q: Query<(&mut Transform, &Boss, &BossEnergy, &mut BossTarget)>,
    mut rng: Local<Option<SmallRng>>,
) {
    let rng = rng.get_or_insert_with(SmallRng::from_os_rng);
    let dt = time.delta_secs();

    let safe_min_y = (-screen.half_height + 260.0).min(screen.half_height - 200.0);
    let top_y = screen.half_height - 120.0;

    for (mut tr, boss, energy, mut target) in q.iter_mut() {
        target.timer.tick(time.delta());

        let frac = if energy.max > 0 {
            (energy.current as f32 / energy.max as f32).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let stage = boss.stage.max(1) as f32;
        let speed = 120.0 + stage * 14.0 + (1.0 - frac) * 40.0;

        let pos = tr.translation.truncate();
        if target.timer.is_finished() || pos.distance(target.target) < 24.0 {
            // Pick a fresh target frequently; dip lower as HP drops, but never near the player.
            let dip = (1.0 - frac).powf(1.6);
            let y_min = (top_y - 180.0 * dip).max(safe_min_y);
            let y_max = top_y;
            let x_min = -screen.half_width + 140.0;
            let x_max = screen.half_width - 140.0;
            let x = if x_min < x_max {
                rng.random_range(x_min..x_max)
            } else {
                0.0
            };
            let y = if y_min < y_max {
                rng.random_range(y_min..y_max)
            } else {
                y_max
            };

            target.target = Vec2::new(x, y);
            let cadence = 0.7 + rng.random::<f32>() * 0.55;
            target.timer =
                Timer::from_seconds((cadence / (1.0 + stage * 0.06)).max(0.35), TimerMode::Once);
        }

        let to = target.target - pos;
        let dir = to.normalize_or_zero();
        let step = (speed * dt).min(to.length());
        tr.translation.x += dir.x * step;
        tr.translation.y = (tr.translation.y + dir.y * step).clamp(safe_min_y, top_y);
    }
}

pub fn boss_escort_movement(
    time: Res<Time>,
    boss: Query<&Transform, (With<Boss>, Without<BossEscort>)>,
    mut escorts: Query<(&mut Transform, &BossEscort), Without<Boss>>,
) {
    let Ok(boss_t) = boss.single() else { return };
    let t = time.elapsed_secs();

    for (mut tr, escort) in escorts.iter_mut() {
        let a = escort.angle + t * escort.speed;
        let x = boss_t.translation.x + a.cos() * escort.radius;
        let y = boss_t.translation.y - 14.0 + a.sin() * escort.radius * 0.35;
        tr.translation.x = x;
        tr.translation.y = y;
        tr.translation.z = boss_t.translation.z + 0.1;
    }
}

#[allow(clippy::too_many_arguments)]
pub fn boss_escort_shooting(
    mut cmd: Commands,
    time: Res<Time>,
    sprites: Res<SpriteAssets>,
    boss_q: Query<(&Boss, &BossEnergy), With<Boss>>,
    player: Query<&Transform, With<Player>>,
    enemy_bullets: Query<Entity, With<EnemyBullet>>,
    mut escorts: Query<(&Transform, &mut EscortShooter), With<BossEscort>>,
    mut rng: Local<Option<SmallRng>>,
) {
    let Ok((boss, energy)) = boss_q.single() else {
        return;
    };
    if boss.stage < 2 {
        return;
    }
    let Ok(pt) = player.single() else { return };
    let rng = rng.get_or_insert_with(SmallRng::from_os_rng);

    let frac = if energy.max > 0 {
        (energy.current as f32 / energy.max as f32).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let stage = boss.stage.max(1) as f32;

    let global_cap = (12 + (boss.stage as usize).min(8) * 2).min(26);
    if enemy_bullets.iter().count() >= global_cap {
        return;
    }

    let player_x = pt.translation.x;

    for (t, mut shooter) in escorts.iter_mut() {
        shooter.timer.tick(time.delta());
        if !shooter.timer.just_finished() {
            continue;
        }

        // Staggered, slightly random cadence so escorts feel alive.
        let base = (1.35 - stage * 0.08).max(0.55);
        let phase_boost = 0.75 + (1.0 - frac) * 0.65;
        shooter.timer = Timer::from_seconds(
            (base / phase_boost) + 0.12 + rng.random::<f32>() * 0.22,
            TimerMode::Once,
        );

        // Chance gate so it's threatening but not constant wall.
        let chance = (0.35 + stage * 0.05 + (1.0 - frac) * 0.10).min(0.70);
        if rng.random::<f32>() > chance {
            continue;
        }

        if enemy_bullets.iter().count() >= global_cap {
            break;
        }

        let origin = Vec3::new(t.translation.x, t.translation.y - ENEMY_SIZE.y * 0.35, 1.0);
        let bullet_idx = rng.random_range(0..3);

        let dx = (player_x - origin.x).clamp(-320.0, 320.0);
        let vx = (dx * (0.85 + (1.0 - frac) * 0.45)).clamp(-240.0, 240.0);
        let vy = -((ENEMY_BULLET_SPEED * 1.05).min(460.0));

        // Occasionally fire a small spread.
        let spread = if boss.stage >= 4 && rng.random::<f32>() < 0.35 {
            Some(120.0)
        } else {
            None
        };

        let spawn = |cmd: &mut Commands, vx: f32| {
            cmd.spawn((
                Sprite::from_atlas_image(
                    sprites.enemy_bullet_texture.clone(),
                    TextureAtlas {
                        layout: sprites.enemy_bullet_layout.clone(),
                        index: bullet_idx,
                    },
                ),
                Transform::from_translation(origin).with_scale(Vec3::splat(SPRITE_SCALE * 0.5)),
                EnemyBullet,
                BulletVelocity(Vec2::new(vx, vy)),
                TrailEmitter {
                    timer: Timer::from_seconds(0.045, TimerMode::Repeating),
                    alpha: 0.22,
                },
                GameEntity,
            ));
        };

        spawn(&mut cmd, vx);
        if let Some(s) = spread {
            spawn(&mut cmd, vx - s);
            spawn(&mut cmd, vx + s);
        }
    }
}

pub fn boss_shooting(
    mut cmd: Commands,
    time: Res<Time>,
    sprites: Res<SpriteAssets>,
    mut boss_q: Query<(&Transform, &Boss, &BossEnergy, &mut BossWeapon)>,
    player: Query<&Transform, With<Player>>,
    enemy_bullets: Query<Entity, With<EnemyBullet>>,
    mut rng: Local<Option<SmallRng>>,
) {
    let Ok((boss_t, boss, energy, mut weapon)) = boss_q.single_mut() else {
        return;
    };
    let rng = rng.get_or_insert_with(SmallRng::from_os_rng);

    weapon.timer.tick(time.delta());
    if !weapon.timer.is_finished() {
        return;
    }

    let stage = boss.stage.max(1);
    let frac = if energy.max > 0 {
        (energy.current as f32 / energy.max as f32).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let cap = (8 + (stage as usize).min(8) * 2).min(22);
    if enemy_bullets.iter().count() >= cap {
        weapon.timer = Timer::from_seconds(0.35 + rng.random::<f32>() * 0.25, TimerMode::Once);
        return;
    }

    let player_x = player.iter().next().map(|t| t.translation.x).unwrap_or(0.0);
    let origin = Vec3::new(
        boss_t.translation.x,
        boss_t.translation.y - BOSS_HIT_RADIUS,
        1.0,
    );

    let bullet_idx = rng.random_range(0..3);
    let vy = -(ENEMY_BULLET_SPEED * (1.1 + (stage as f32).min(8.0) * 0.03 + (1.0 - frac) * 0.08));
    let dx = (player_x - origin.x).clamp(-260.0, 260.0);
    let aimed_vx = (dx * (1.05 + (1.0 - frac) * 0.35)).clamp(-260.0, 260.0);

    let mut spawn_bullet = |pos: Vec3, vx: f32| {
        cmd.spawn((
            Sprite::from_atlas_image(
                sprites.enemy_bullet_texture.clone(),
                TextureAtlas {
                    layout: sprites.enemy_bullet_layout.clone(),
                    index: bullet_idx,
                },
            ),
            Transform::from_translation(pos).with_scale(Vec3::splat(SPRITE_SCALE * 0.55)),
            EnemyBullet,
            BulletVelocity(Vec2::new(vx, vy)),
            TrailEmitter {
                timer: Timer::from_seconds(0.045, TimerMode::Repeating),
                alpha: 0.22,
            },
            GameEntity,
        ));
    };

    // Boss patterns:
    // - Phase 1 (high HP): aimed triples.
    // - Phase 2 (mid HP): adds a sweeping fan.
    // - Phase 3 (low HP): occasional "curtain" shots.

    // Aimed triple (always).
    let spread = 95.0 + (1.0 - frac) * 35.0;
    spawn_bullet(origin, aimed_vx);
    spawn_bullet(origin + Vec3::new(-16.0, 0.0, 0.0), aimed_vx - spread);
    spawn_bullet(origin + Vec3::new(16.0, 0.0, 0.0), aimed_vx + spread);

    // Sweeping fan (often from stage 2+, or when HP < 70%).
    if stage >= 2 || frac < 0.7 {
        let fan_n = if stage >= 4 { 5 } else { 3 };
        let base = aimed_vx * 0.25;
        let step = 140.0;
        for i in 0..fan_n {
            let k = i as f32 - (fan_n as f32 - 1.0) * 0.5;
            spawn_bullet(origin + Vec3::new(0.0, 8.0, 0.0), base + k * step);
        }
    }

    // Curtain (low HP): semi-random left/right walls to force movement.
    if frac < 0.38 && rng.random::<f32>() < 0.55 {
        let wall = 220.0 + rng.random::<f32>() * 60.0;
        spawn_bullet(origin + Vec3::new(-28.0, 10.0, 0.0), -wall);
        spawn_bullet(origin + Vec3::new(28.0, 10.0, 0.0), wall);
    }

    // Variable cooldown (faster as HP drops, but with jitter).
    let base = (0.85 - (stage as f32 - 1.0) * 0.06).max(0.48);
    let phase_boost = 0.35 + (1.0 - frac) * 0.55;
    let jitter = 0.10 + rng.random::<f32>() * 0.18;
    weapon.timer = Timer::from_seconds((base / phase_boost + jitter).max(0.30), TimerMode::Once);
}

#[allow(clippy::too_many_arguments)]
pub fn enemy_shooting(
    mut cmd: Commands,
    time: Res<Time>,
    gd: Res<GameData>,
    mut timers: ResMut<GameTimers>,
    sprites: Res<SpriteAssets>,
    q: Query<&Transform, With<Enemy>>,
    player: Query<&Transform, With<Player>>,
    enemy_bullets: Query<Entity, With<EnemyBullet>>,
    mut rng: Local<Option<SmallRng>>,
    mut enemies_cache: Local<Vec<EnemyColumnPos>>,
) {
    let rng = rng.get_or_insert_with(SmallRng::from_os_rng);
    timers.enemy_shoot.tick(time.delta());
    if timers.enemy_shoot.is_finished() {
        let player_x = player.iter().next().map(|t| t.translation.x).unwrap_or(0.0);
        let column_w = ENEMY_SPACING.x;
        enemies_cache.clear();
        for t in q.iter() {
            enemies_cache.push(EnemyColumnPos {
                col: (t.translation.x / column_w).round() as i32,
                x: t.translation.x,
                y: t.translation.y,
            });
        }
        let enemies = enemies_cache.as_slice();
        let bullet_cap = (4 + (gd.wave as usize / 3).min(8)).min(12);
        if enemy_bullets.iter().count() >= bullet_cap {
            timers.enemy_shoot = Timer::from_seconds(gd.enemy_shoot_rate(), TimerMode::Once);
            return;
        }
        if !enemies.is_empty() {
            // Galaxian-like: pick a column near the player, then shoot from the lowest enemy in that column.
            let mut best_col = enemies[rng.random_range(0..enemies.len())].col;
            let mut best_col_score = (((best_col as f32) * column_w) - player_x).abs();
            for _ in 0..enemies.len().min(10) {
                let cand_col = enemies[rng.random_range(0..enemies.len())].col;
                let score = ((((cand_col as f32) * column_w) - player_x).abs())
                    + rng.random::<f32>() * 18.0;
                if score < best_col_score {
                    best_col = cand_col;
                    best_col_score = score;
                }
            }

            let mut shooter: Option<EnemyColumnPos> = None;
            for &ep in enemies {
                if ep.col != best_col {
                    continue;
                }
                if shooter.is_none_or(|s| ep.y < s.y) {
                    shooter = Some(ep);
                }
            }

            if let Some(t) = shooter {
                let bullet_idx = rng.random_range(0..3);
                // Add light horizontal lead in later waves so the player can't just sit still.
                let aimed = gd.wave >= 5 && rng.random::<f32>() < 0.55;
                let dx = (player_x - t.x).clamp(-240.0, 240.0);
                let vx = (dx * 0.85).clamp(-150.0, 150.0);
                let vy = -((ENEMY_BULLET_SPEED
                    * (1.0 + (gd.wave.saturating_sub(1) as f32) * 0.01))
                    .min(420.0));

                let mut ent = cmd.spawn((
                    Sprite::from_atlas_image(
                        sprites.enemy_bullet_texture.clone(),
                        TextureAtlas {
                            layout: sprites.enemy_bullet_layout.clone(),
                            index: bullet_idx,
                        },
                    ),
                    Transform::from_xyz(t.x, t.y - ENEMY_SIZE.y / 2.0, 1.0)
                        .with_scale(Vec3::splat(SPRITE_SCALE * 0.5)),
                    EnemyBullet,
                    TrailEmitter {
                        timer: Timer::from_seconds(0.045, TimerMode::Repeating),
                        alpha: 0.22,
                    },
                    GameEntity,
                ));
                if aimed {
                    ent.insert(BulletVelocity(Vec2::new(vx, vy)));
                }
            }
        }
        let remaining = enemies.len();
        let late_wave_mult = if remaining <= 8 { 0.78 } else { 1.0 };
        timers.enemy_shoot =
            Timer::from_seconds(gd.enemy_shoot_rate() * late_wave_mult, TimerMode::Once);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn initiate_dives(
    mut cmd: Commands,
    time: Res<Time>,
    gd: Res<GameData>,
    mut timers: ResMut<GameTimers>,
    eq: FormationEnemyQuery,
    dq: Query<&DivingEnemy>,
    pq: Query<&Transform, With<Player>>,
    mut rng: Local<Option<SmallRng>>,
) {
    let rng = rng.get_or_insert_with(SmallRng::from_os_rng);
    timers.dive.tick(time.delta());
    if timers.dive.is_finished() {
        // Fewer enemies alive => more frequent attacks (Galaxian end-of-wave pressure).
        let remaining = eq.iter().count();
        let late_wave_mult = if remaining <= 8 { 0.75 } else { 1.0 };
        timers.dive = Timer::from_seconds(gd.dive_interval() * late_wave_mult, TimerMode::Once);
        let (current, max) = (dq.iter().count(), gd.max_divers());
        if current >= max {
            return;
        }
        let Ok(pt) = pq.single() else { return };
        let candidates: Vec<_> = eq
            .iter()
            // Prefer upper ships for dives (more Galaxian-like).
            .filter(|(_, t)| t.translation.y > 120.0)
            .collect();
        if candidates.is_empty() {
            return;
        }
        let batch = (max - current)
            .min(1 + gd.wave as usize / 6)
            .min(candidates.len());
        for _ in 0..batch {
            // Pick a diver biased toward the player's x position.
            let mut best = candidates[rng.random_range(0..candidates.len())];
            let mut best_score = (best.1.translation.x - pt.translation.x).abs();
            for _ in 0..candidates.len().min(10) {
                let cand = candidates[rng.random_range(0..candidates.len())];
                let score =
                    (cand.1.translation.x - pt.translation.x).abs() + rng.random::<f32>() * 22.0;
                if score < best_score {
                    best = cand;
                    best_score = score;
                }
            }
            let (e, t) = best;
            cmd.entity(e).insert((
                DivingEnemy {
                    target_x: pt.translation.x + rng.random_range(-80.0..80.0),
                    returning: false,
                    start_y: t.translation.y,
                    original_x: t.translation.x,
                    progress: 0.0,
                    amplitude: rng.random_range(80.0..160.0),
                    curve_dir: if rng.random_bool(0.5) { 1.0 } else { -1.0 },
                },
                DiveShooter {
                    timer: Timer::from_seconds(0.35 + rng.random::<f32>() * 0.55, TimerMode::Once),
                },
            ));
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn diver_shooting(
    mut cmd: Commands,
    time: Res<Time>,
    gd: Res<GameData>,
    sprites: Res<SpriteAssets>,
    player: Query<&Transform, With<Player>>,
    enemy_bullets: Query<Entity, With<EnemyBullet>>,
    mut divers: Query<(&Transform, &mut DiveShooter), With<DivingEnemy>>,
    mut rng: Local<Option<SmallRng>>,
) {
    let Ok(pt) = player.single() else { return };
    let rng = rng.get_or_insert_with(SmallRng::from_os_rng);
    let player_x = pt.translation.x;

    // Divers add pressure but keep an upper cap to avoid bullet soup.
    let cap = (7 + (gd.wave as usize / 2).min(10)).min(18);
    if enemy_bullets.iter().count() >= cap {
        return;
    }

    for (t, mut shooter) in divers.iter_mut() {
        shooter.timer.tick(time.delta());
        if !shooter.timer.just_finished() {
            continue;
        }

        // Not every tick shoots: chance increases with wave.
        let chance = (0.25 + (gd.wave.saturating_sub(1) as f32) * 0.03).min(0.65);
        let cooldown = (1.25 - (gd.wave.saturating_sub(1) as f32) * 0.05).max(0.55);
        shooter.timer = Timer::from_seconds(cooldown + rng.random::<f32>() * 0.25, TimerMode::Once);

        if rng.random::<f32>() > chance {
            continue;
        }

        let origin = Vec3::new(t.translation.x, t.translation.y - ENEMY_SIZE.y * 0.35, 1.0);
        let bullet_idx = rng.random_range(0..3);

        let dx = (player_x - origin.x).clamp(-260.0, 260.0);
        let vx = (dx * 1.05).clamp(-220.0, 220.0);
        let vy = -((ENEMY_BULLET_SPEED * 1.15).min(460.0));

        cmd.spawn((
            Sprite::from_atlas_image(
                sprites.enemy_bullet_texture.clone(),
                TextureAtlas {
                    layout: sprites.enemy_bullet_layout.clone(),
                    index: bullet_idx,
                },
            ),
            Transform::from_translation(origin).with_scale(Vec3::splat(SPRITE_SCALE * 0.5)),
            EnemyBullet,
            BulletVelocity(Vec2::new(vx, vy)),
            TrailEmitter {
                timer: Timer::from_seconds(0.045, TimerMode::Repeating),
                alpha: 0.22,
            },
            GameEntity,
        ));
    }
}

pub fn diving_movement(
    mut cmd: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut Transform, &mut DivingEnemy)>,
    screen: Res<ScreenSize>,
    gd: Res<GameData>,
) {
    // Keep divers above the player zone (player is at -hh + 60).
    let bottom = (-screen.half_height + 210.0).min(screen.half_height - 180.0);
    let dt = time.delta_secs();
    let fwd_speed = (0.9 + (gd.wave.saturating_sub(1) as f32) * 0.02).min(1.25);
    let ret_speed = (0.7 + (gd.wave.saturating_sub(1) as f32) * 0.015).min(1.05);

    for (e, mut t, mut d) in q.iter_mut() {
        if !d.returning {
            d.progress += dt * fwd_speed;
            let total_drop = d.start_y - bottom;
            let target_y = d.start_y - d.progress * total_drop;
            let wave = (d.progress * std::f32::consts::PI * 2.0).sin();
            let base_x = d.original_x + (d.target_x - d.original_x) * d.progress;
            t.translation.x = base_x + wave * d.amplitude * d.curve_dir;
            t.translation.y = target_y;
            if d.progress >= 1.0 {
                d.returning = true;
                d.progress = 0.0;
            }
        } else {
            d.progress += dt * ret_speed;
            let p = d.progress.min(1.0);
            let ease = 1.0 - (1.0 - p).powi(2);
            let arc = (p * std::f32::consts::PI).sin();
            let target_x = d.target_x + (d.original_x - d.target_x) * ease;
            t.translation.x = target_x + arc * d.amplitude * 0.5 * -d.curve_dir;
            t.translation.y = bottom + (d.start_y - bottom) * ease;
            if d.progress >= 1.0 {
                t.translation = Vec3::new(d.original_x, d.start_y, 1.0);
                cmd.entity(e).remove::<(DivingEnemy, DiveShooter)>();
            }
        }
    }
}

pub fn spiral_movement(
    mut cmd: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut Transform, &mut SpiralEnemy)>,
    screen: Res<ScreenSize>,
    gd: Res<GameData>,
) {
    // Keep attackers above the player zone.
    let bottom = (-screen.half_height + 240.0).min(screen.half_height - 200.0);
    let dt = time.delta_secs();

    // Speed ramps a bit with wave, but stays readable.
    let wave_speed = (0.75 + (gd.wave.saturating_sub(1) as f32) * 0.02).min(1.15);

    for (e, mut t, mut s) in q.iter_mut() {
        if !s.returning {
            s.progress += dt * wave_speed;
            let p = s.progress.min(1.0);

            let total_drop = s.start_y - bottom;
            let base_y = s.start_y - p * total_drop;
            let base_x = s.original.x + (s.target_x - s.original.x) * p;

            let radius = (s.radius * (1.0 - p)).max(0.0);
            let angle = (p * s.turns * std::f32::consts::TAU) * s.curve_dir;
            let wobble = (time.elapsed_secs() * s.wobble_freq).sin() * s.wobble_amp;

            t.translation.x = base_x + angle.cos() * radius + wobble;
            t.translation.y = base_y + angle.sin() * radius * 0.22;

            if s.progress >= 1.0 {
                s.returning = true;
                s.progress = 0.0;
                // Re-aim slightly so returns don't stack perfectly.
                s.target_x = s.original.x;
            }
        } else {
            s.progress += dt * (wave_speed * 0.85);
            let p = s.progress.min(1.0);
            let ease = 1.0 - (1.0 - p).powi(2);

            // Return arc: inverse spiral back into the original slot.
            let base_x = t.translation.x + (s.original.x - t.translation.x) * (ease * 0.55);
            let base_y = bottom + (s.original.y - bottom) * ease;
            let radius = (s.radius * (1.0 - ease)).max(0.0);
            let angle = ((1.0 - ease) * s.turns * std::f32::consts::TAU) * -s.curve_dir;

            t.translation.x = base_x + angle.sin() * radius * 0.35;
            t.translation.y = base_y + angle.cos() * radius * 0.12;

            if s.progress >= 1.0 {
                t.translation = Vec3::new(s.original.x, s.original.y, 1.0);
                cmd.entity(e).remove::<SpiralEnemy>();
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn boss_death_fx_update(
    mut cmd: Commands,
    time: Res<Time>,
    sprites: Res<SpriteAssets>,
    mut shake: ResMut<ScreenShake>,
    mut flash: ResMut<ScreenFlash>,
    mut sfx: MessageWriter<PlaySfxMsg>,
    mut q: Query<(Entity, &mut BossDeathFx)>,
    mut rng: Local<Option<SmallRng>>,
) {
    if q.is_empty() {
        return;
    }
    let rng = rng.get_or_insert_with(SmallRng::from_os_rng);

    for (entity, mut fx) in q.iter_mut() {
        fx.pulse.tick(time.delta());

        let mut bursts = fx.pulse.times_finished_this_tick().min(fx.remaining as u32) as u8;
        while bursts > 0 && fx.remaining > 0 {
            bursts -= 1;

            let step = fx.total.saturating_sub(fx.remaining);
            let t = if fx.total > 0 {
                step as f32 / fx.total as f32
            } else {
                0.0
            };

            // Two explosions per pulse, spreading outward over time.
            for _ in 0..2 {
                let a = rng.random_range(0.0..std::f32::consts::TAU);
                let r = 18.0 + t * 140.0 + rng.random::<f32>() * 18.0;
                let offset = Vec3::new(a.cos() * r, a.sin() * r * 0.75, 0.0);
                spawn_explosion(&mut cmd, fx.origin + offset, &sprites);
            }

            // Stronger shake early, slightly less later.
            let trauma = (0.45 - t * 0.18).max(0.22);
            shake.add_trauma(trauma);

            // Flash pulses (still clamped in screen_flash_system, but repeated).
            let flash_color = if step % 2 == 0 {
                Color::srgba(1.0, 0.85, 0.25, 1.0)
            } else {
                Color::srgba(1.0, 1.0, 1.0, 0.9)
            };
            flash.trigger(0.22, flash_color);

            // Multiple explosion hits for the GIST sound.
            sfx.write(PlaySfxMsg(SfxType::Explode));
            if fx.remaining == fx.total || fx.remaining <= 2 {
                sfx.write(PlaySfxMsg(SfxType::Explode));
            }

            fx.remaining = fx.remaining.saturating_sub(1);
        }

        if fx.remaining == 0 {
            cmd.entity(entity).try_despawn();
        }
    }
}

// === Collisions ===

#[allow(clippy::too_many_arguments)]
pub fn collisions(
    mut cmd: Commands,
    mut queries: CollisionQueries,
    ctx: ShootingContext,
    mut gd: ResMut<GameData>,
    mut events: CollisionEvents,
    mut vfx: CollisionVfx,
    mut extra: MessageWriter<ExtraLifeMsg>,
    mut sfx: MessageWriter<PlaySfxMsg>,
    mut boss_q: Query<(Entity, &Transform, &Sprite, &Boss, &mut BossEnergy), With<Boss>>,
    boss_bars: BossUiQuery<'_, '_>,
    boss_escorts: Query<Entity, With<BossEscort>>,
    invincible_player: Query<(), (With<Player>, With<Invincible>)>,
    powerups_on_screen: Query<&PowerUp>,
    mut energy: ResMut<PlayerEnergy>,
    mut rng: Local<Option<SmallRng>>,
) {
    let rng = rng.get_or_insert_with(SmallRng::from_os_rng);
    let player_is_invincible = !invincible_player.is_empty();

    for (be, bt, damage) in queries.player_bullets.iter() {
        let bp = bt.translation.truncate();

        // Boss hit check first (so bullets don't "pass through" into the formation).
        if let Ok((boss_entity, boss_t, boss_sprite, boss, mut boss_energy)) = boss_q.single_mut()
            && boss_energy.current > 0
            && bp.distance(boss_t.translation.truncate()) < BULLET_SIZE.y / 2.0 + BOSS_HIT_RADIUS
        {
            cmd.entity(be).try_despawn();
            let dmg = damage.map(|d| d.0 as i32).unwrap_or(1);
            boss_energy.current = boss_energy.current.saturating_sub(dmg);

            // Subtle boss hit flash (avoid harsh screen flashes).
            let mut flash_sprite = boss_sprite.clone();
            flash_sprite.color = Color::srgba(1.0, 1.0, 1.0, 0.22);
            let base_scale = boss_t.scale * 1.02;
            cmd.spawn((
                flash_sprite,
                Transform::from_translation(boss_t.translation + Vec3::new(0.0, 0.0, 1.2))
                    .with_scale(base_scale),
                HitFlash {
                    timer: Timer::from_seconds(0.06, TimerMode::Once),
                    start_alpha: 0.22,
                    base_scale,
                },
                GameEntity,
            ));
            vfx.shake.add_trauma(SHAKE_TRAUMA_EXPLOSION * 0.22);

            if boss_energy.current <= 0 {
                // Prevent double-kills within the same frame (despawn is deferred).
                boss_energy.current = -999;

                // Big, readable boss destruction without extreme flashing.
                spawn_boss_death_explosions(&mut cmd, boss_t.translation, &ctx.sprites);
                for e in boss_escorts.iter() {
                    cmd.entity(e).try_despawn();
                }
                for e in boss_bars.iter() {
                    cmd.entity(e).try_despawn();
                }
                cmd.entity(boss_entity).try_despawn();
                events.enemy_killed.write(EnemyKilledMsg(boss.points));
                spawn_score_popup(&mut cmd, &vfx.fonts, boss_t.translation, boss.points, 1);
                vfx.shake.add_trauma(SHAKE_TRAUMA_EXPLOSION * 0.9);
                vfx.flash.trigger(0.18, Color::srgba(1.0, 0.65, 0.25, 0.10));

                // Boss clear reward: refill player energy and give a score bonus.
                energy.reset();
                let bonus = 1500 + (boss.stage * 900);
                award_points(&mut gd, bonus, &mut extra);
                spawn_fading_text(
                    &mut cmd,
                    &vfx.fonts,
                    &format!("BONUS  {bonus}"),
                    1.6,
                    Color::srgba(0.35, 1.0, 0.55, 1.0),
                    false,
                );
                vfx.flash.trigger(0.18, Color::srgba(0.35, 1.0, 0.55, 0.10));

                // Extra boss-death spectacle: delayed explosion pulses + extra shake + multiple SFX hits.
                cmd.spawn((
                    BossDeathFx {
                        origin: boss_t.translation,
                        pulse: Timer::from_seconds(0.12, TimerMode::Repeating),
                        remaining: 10,
                        total: 10,
                    },
                    GameEntity,
                ));
                // Immediate SFX burst.
                sfx.write(PlaySfxMsg(SfxType::Explode));
                sfx.write(PlaySfxMsg(SfxType::Explode));
            }
            continue;
        }

        for (ee, et, enemy_sprite, enemy, escort, mut enemy_hp) in queries.enemies.iter_mut() {
            if bp.distance(et.translation.truncate()) < BULLET_SIZE.y / 2.0 + ENEMY_SIZE.x / 2.0 {
                cmd.entity(be).try_despawn();

                let dmg = damage.map(|d| d.0).unwrap_or(1);
                let remaining = enemy_hp.current.saturating_sub(dmg);
                if remaining > 0 {
                    enemy_hp.current = remaining;
                    // Non-lethal hit feedback (no explosion, keeps the screen readable).
                    let mut flash_sprite = enemy_sprite.clone();
                    flash_sprite.color = Color::srgba(1.0, 1.0, 1.0, 0.35);
                    let base_scale = et.scale * 1.03;
                    cmd.spawn((
                        flash_sprite,
                        Transform::from_translation(et.translation + Vec3::new(0.0, 0.0, 1.2))
                            .with_scale(base_scale),
                        HitFlash {
                            timer: Timer::from_seconds(0.05, TimerMode::Once),
                            start_alpha: 0.35,
                            base_scale,
                        },
                        GameEntity,
                    ));
                    vfx.shake.add_trauma(SHAKE_TRAUMA_EXPLOSION * 0.08);
                    break;
                }

                cmd.entity(ee).try_despawn();

                // Bright hit flash (palette shift + slight scale) for immediate readability.
                let mut flash_sprite = enemy_sprite.clone();
                flash_sprite.color = Color::srgba(1.0, 1.0, 1.0, 0.55);
                let base_scale = et.scale * 1.05;
                cmd.spawn((
                    flash_sprite,
                    Transform::from_translation(et.translation + Vec3::new(0.0, 0.0, 1.2))
                        .with_scale(base_scale),
                    HitFlash {
                        timer: Timer::from_seconds(0.06, TimerMode::Once),
                        start_alpha: 0.55,
                        base_scale,
                    },
                    GameEntity,
                ));

                spawn_explosion(&mut cmd, et.translation, &ctx.sprites);
                events.enemy_killed.write(EnemyKilledMsg(enemy.points));

                // Track combo and spawn popup
                vfx.combo.add_kill();
                vfx.shake.add_trauma(SHAKE_TRAUMA_EXPLOSION);

                // Tiny flash on explosions to sell impact (also drives CRT).
                vfx.flash.trigger(0.12, Color::srgba(1.0, 0.85, 0.3, 0.14));

                spawn_score_popup(
                    &mut cmd,
                    &vfx.fonts,
                    et.translation,
                    enemy.points,
                    vfx.combo.current(),
                );

                let mut on_screen_kinds: [PowerUpType; 2] = [PowerUpType::Heal, PowerUpType::Heal];
                let mut kinds_len = 0usize;
                let mut on_screen_count = 0usize;
                for p in powerups_on_screen.iter() {
                    on_screen_count += 1;
                    if kinds_len < 2 && !on_screen_kinds[..kinds_len].contains(&p.kind) {
                        on_screen_kinds[kinds_len] = p.kind;
                        kinds_len += 1;
                    }
                }
                let on_screen_kinds = &on_screen_kinds[..kinds_len];

                // Random chance to spawn power-up (cap: max 2, different kinds).
                // Boss escorts never drop power-ups to keep drops readable/rare.
                if escort.is_none()
                    && on_screen_count < 2
                    && rng.random::<f32>() < POWERUP_DROP_CHANCE
                {
                    spawn_powerup(
                        &mut cmd,
                        et.translation,
                        &ctx.sprites,
                        &ctx.powerups,
                        energy.as_ref(),
                        on_screen_kinds,
                        rng,
                    );
                }
                break;
            }
        }
    }

    // Skip player-hit collisions if invincible
    if player_is_invincible {
        return;
    }

    let Ok((player_entity, pt)) = queries.player.single() else {
        return;
    };
    let pp = pt.translation.truncate();

    for (be, bt) in queries.enemy_bullets.iter() {
        if bt.translation.truncate().distance(pp) < BULLET_SIZE.y / 2.0 + PLAYER_SIZE.x / 2.0 {
            cmd.entity(be).try_despawn();
            let depleted = energy.take_hit();
            if depleted {
                // Multiple explosions for dramatic player death
                spawn_player_death_explosions(&mut cmd, pt.translation, &ctx.sprites);
                vfx.shake.add_trauma(SHAKE_TRAUMA_PLAYER_HIT);
                events.player_hit.write(PlayerHitMsg);
                return;
            }

            cmd.entity(player_entity).insert(Invincible::new(0.65));
            cmd.spawn((
                Sprite {
                    image: ctx.sprites.bubble_texture.clone(),
                    color: Color::srgba(1.0, 0.3, 0.3, 0.35),
                    ..default()
                },
                Transform::from_xyz(pt.translation.x, pt.translation.y, 0.9),
                ShieldBubble,
                GameEntity,
            ));
            vfx.shake.add_trauma(SHAKE_TRAUMA_PLAYER_HIT * 0.18);
            vfx.flash.trigger(0.10, Color::srgba(1.0, 0.2, 0.2, 0.10));
            return;
        }
    }

    for (de, dt) in queries.divers.iter() {
        if dt.translation.truncate().distance(pp) < ENEMY_SIZE.x / 2.0 + PLAYER_SIZE.x / 2.0 {
            cmd.entity(de).try_despawn();
            let depleted = energy.take_hit();
            if depleted {
                // Multiple explosions for dramatic player death
                spawn_player_death_explosions(&mut cmd, pt.translation, &ctx.sprites);
                vfx.shake.add_trauma(SHAKE_TRAUMA_PLAYER_HIT);
                events.player_hit.write(PlayerHitMsg);
                return;
            }

            cmd.entity(player_entity).insert(Invincible::new(0.75));
            cmd.spawn((
                Sprite {
                    image: ctx.sprites.bubble_texture.clone(),
                    color: Color::srgba(1.0, 0.3, 0.3, 0.35),
                    ..default()
                },
                Transform::from_xyz(pt.translation.x, pt.translation.y, 0.9),
                ShieldBubble,
                GameEntity,
            ));
            vfx.shake.add_trauma(SHAKE_TRAUMA_PLAYER_HIT * 0.22);
            vfx.flash.trigger(0.10, Color::srgba(1.0, 0.2, 0.2, 0.10));
            return;
        }
    }
}

/// Spawn multiple explosions for a dramatic player death effect
fn spawn_player_death_explosions(cmd: &mut Commands, pos: Vec3, sprites: &SpriteAssets) {
    // Center explosion
    spawn_explosion(cmd, pos, sprites);
    // Offset explosions for cluster effect
    let offsets = [
        Vec3::new(-15.0, 10.0, 0.0),
        Vec3::new(15.0, 10.0, 0.0),
        Vec3::new(0.0, -12.0, 0.0),
    ];
    for offset in offsets {
        spawn_explosion(cmd, pos + offset, sprites);
    }
}

fn spawn_boss_death_explosions(cmd: &mut Commands, pos: Vec3, sprites: &SpriteAssets) {
    // Larger cluster + multiple ring bursts for scale.
    spawn_explosion(cmd, pos, sprites);
    let offsets = [
        Vec3::new(-28.0, 16.0, 0.0),
        Vec3::new(28.0, 16.0, 0.0),
        Vec3::new(-22.0, -18.0, 0.0),
        Vec3::new(22.0, -18.0, 0.0),
        Vec3::new(0.0, 28.0, 0.0),
        Vec3::new(0.0, -30.0, 0.0),
        Vec3::new(-44.0, 4.0, 0.0),
        Vec3::new(44.0, 4.0, 0.0),
        Vec3::new(-34.0, -32.0, 0.0),
        Vec3::new(34.0, -32.0, 0.0),
        Vec3::new(-10.0, 44.0, 0.0),
        Vec3::new(10.0, 44.0, 0.0),
    ];
    for offset in offsets {
        spawn_explosion(cmd, pos + offset, sprites);
    }
}

/// Update combo timer
pub fn combo_tick(time: Res<Time>, mut combo: ResMut<ComboTracker>) {
    combo.tick(time.delta_secs());
}

pub fn energy_bar_update(
    time: Res<Time>,
    energy: Res<PlayerEnergy>,
    mut q: Query<(&mut Sprite, &mut Transform, &EnergyBarFill)>,
) {
    let frac = energy.fraction();
    let is_critical = frac <= 0.33 && frac > 0.0;

    // Only update when energy changes, unless we need to pulse for critical health
    if !energy.is_changed() && !is_critical {
        return;
    }

    let base_color = if frac > 0.66 {
        Color::srgb(0.35, 1.0, 0.45)
    } else if frac > 0.33 {
        Color::srgb(1.0, 0.9, 0.35)
    } else {
        Color::srgb(1.0, 0.35, 0.35)
    };

    // Critical health pulse effect - flash between red and white
    let color = if is_critical {
        let t = time.elapsed_secs();
        let pulse = ((t * 8.0).sin() + 1.0) * 0.5; // 0.0 - 1.0
        let base = base_color.to_srgba();
        Color::srgb(
            base.red + (1.0 - base.red) * pulse * 0.5,
            base.green + (1.0 - base.green) * pulse * 0.3,
            base.blue + (1.0 - base.blue) * pulse * 0.3,
        )
    } else {
        base_color
    };

    for (mut sprite, mut transform, info) in q.iter_mut() {
        let fill_width = (frac * info.width).clamp(0.0, info.width);
        sprite.custom_size = Some(Vec2::new(fill_width.min(info.width), info.height));
        sprite.color = color;
        transform.translation.x = info.left_x + fill_width * 0.5;
    }
}

pub fn boss_bar_update(
    boss: Query<&BossEnergy, With<Boss>>,
    mut q: Query<(&mut Sprite, &mut Transform, &BossBarFill)>,
) {
    let Ok(energy) = boss.single() else { return };
    if energy.max <= 0 {
        return;
    }
    let frac = (energy.current as f32 / energy.max as f32).clamp(0.0, 1.0);
    let color = if frac > 0.55 {
        Color::srgb(1.0, 0.55, 0.25)
    } else if frac > 0.25 {
        Color::srgb(1.0, 0.35, 0.35)
    } else {
        Color::srgb(1.0, 0.15, 0.15)
    };

    for (mut sprite, mut transform, info) in q.iter_mut() {
        let fill_width = (frac * info.width).clamp(0.0, info.width);
        sprite.custom_size = Some(Vec2::new(fill_width.min(info.width), info.height));
        sprite.color = color;
        transform.translation.x = info.left_x + fill_width * 0.5;
    }
}

pub fn check_wave_complete(
    eq: Query<Entity, With<Enemy>>,
    bq: Query<Entity, With<Boss>>,
    aq: Query<&FadingText>,
    mut events: MessageWriter<WaveCompleteMsg>,
    intermission: Res<WaveIntermission>,
) {
    if intermission.is_active() {
        return;
    }
    if eq.is_empty() && bq.is_empty() && aq.is_empty() {
        events.write(WaveCompleteMsg);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn spawn_queued_powerups(
    mut cmd: Commands,
    time: Res<Time>,
    sprites: Res<SpriteAssets>,
    screen: Res<ScreenSize>,
    mut queue: ResMut<PowerUpDropQueue>,
    mut cooldown: ResMut<PowerUpSpawnCooldown>,
    intermission: Res<WaveIntermission>,
    on_screen: Query<&PowerUp>,
    player: Query<&Transform, With<Player>>,
    mut rng: Local<Option<SmallRng>>,
) {
    if intermission.is_active() {
        return;
    }
    if queue.0.is_empty() {
        return;
    }

    let mut kinds: [PowerUpType; 2] = [PowerUpType::Heal, PowerUpType::Heal];
    let mut kinds_len = 0usize;
    let mut entities = 0usize;
    for p in on_screen.iter() {
        entities += 1;
        if kinds_len < 2 && !kinds[..kinds_len].contains(&p.kind) {
            kinds[kinds_len] = p.kind;
            kinds_len += 1;
        }
    }
    if entities >= 2 || kinds_len >= 2 {
        return;
    }

    cooldown.0.tick(time.delta());
    if !cooldown.0.is_finished() {
        return;
    }

    let player_x = player.iter().next().map(|t| t.translation.x).unwrap_or(0.0);
    let y = screen.half_height - 50.0;
    let hw = screen.half_width - 40.0;
    let rng = rng.get_or_insert_with(SmallRng::from_os_rng);

    // Spawn at most one per cooldown tick to keep drops rare/readable.
    let mut tries = queue.0.len();
    while tries > 0 {
        let Some(kind) = queue.0.pop_front() else {
            break;
        };
        if kinds[..kinds_len].contains(&kind) {
            queue.0.push_back(kind);
            tries -= 1;
            continue;
        }

        let jitter = rng.random_range(-120.0..120.0);
        let x = (player_x + jitter).clamp(-hw, hw);
        spawn_powerup_kind(&mut cmd, Vec3::new(x, y, 1.0), &sprites, kind);
        // No need to update `kinds_len` here: we always spawn at most one per tick and then `break`.

        // Longer cooldown for heals to keep them rare.
        let base = if kind == PowerUpType::Heal { 7.5 } else { 4.8 };
        cooldown.0 = Timer::from_seconds(base + rng.random::<f32>() * 1.1, TimerMode::Once);
        break;
    }
}

// === Animation ===

pub fn animate_sprites(
    time: Res<Time>,
    mut q: Query<(&AnimationIndices, &mut AnimationTimer, &mut Sprite)>,
) {
    for (indices, mut timer, mut sprite) in q.iter_mut() {
        timer.tick(time.delta());
        if timer.just_finished()
            && let Some(atlas) = &mut sprite.texture_atlas
        {
            atlas.index = if atlas.index >= indices.last {
                indices.first
            } else {
                atlas.index + 1
            };
        }
    }
}

/// Animate power-ups (5x5 grid stepping by 5 for same-column frames)
pub fn powerup_animate(
    time: Res<Time>,
    mut q: Query<(&PowerUpAnimation, &mut AnimationTimer, &mut Sprite)>,
) {
    for (anim, mut timer, mut sprite) in q.iter_mut() {
        timer.tick(time.delta());
        if timer.just_finished()
            && let Some(atlas) = &mut sprite.texture_atlas
        {
            // Step by 5 to stay in the same column (row-major 5x5 grid)
            atlas.index = if atlas.index >= anim.last {
                anim.first
            } else {
                atlas.index + 5
            };
        }
    }
}

/// Dynamic booster visual intensity based on speed boost power-up
pub fn booster_intensity_system(
    time: Res<Time>,
    powerups: Res<PowerUpState>,
    mut boosters: Query<(&mut Transform, &mut Sprite), With<Booster>>,
    mut side_boosters: SideBoosterQuery,
) {
    let t = time.elapsed_secs();
    let has_speed = powerups.speed_boost;

    // Pulsing effect when speed boost is active
    let pulse = if has_speed {
        1.0 + (t * 12.0).sin() * 0.15
    } else {
        1.0
    };

    // Scale and color adjustments
    let scale_mult = if has_speed { 1.35 * pulse } else { 1.0 };
    let color = if has_speed {
        // Cyan-tinted flame with pulsing brightness
        let brightness = 0.85 + (t * 8.0).sin() * 0.15;
        Color::srgba(0.4 * brightness, 0.9 * brightness, 1.0 * brightness, 1.0)
    } else {
        Color::WHITE
    };

    // Update main booster (child of player)
    for (mut transform, mut sprite) in boosters.iter_mut() {
        transform.scale = Vec3::splat(scale_mult);
        sprite.color = color;
    }

    // Update side boosters
    for (mut transform, mut sprite) in side_boosters.iter_mut() {
        transform.scale = Vec3::splat(scale_mult);
        sprite.color = color;
    }
}

pub fn explosion_update(
    mut cmd: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &AnimationIndices, &mut AnimationTimer, &Sprite), With<Explosion>>,
) {
    for (entity, indices, mut timer, sprite) in q.iter_mut() {
        timer.tick(time.delta());
        if timer.just_finished()
            && let Some(atlas) = &sprite.texture_atlas
            && atlas.index >= indices.last
        {
            cmd.entity(entity).try_despawn();
        }
    }
}

pub fn explosion_ring_update(
    mut cmd: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut Transform, &mut Sprite, &mut ExplosionRing)>,
) {
    for (entity, mut transform, mut sprite, mut ring) in q.iter_mut() {
        ring.timer.tick(time.delta());
        let progress = ring.timer.fraction().clamp(0.0, 1.0);
        let scale = ring.start_scale + (ring.end_scale - ring.start_scale) * progress;
        transform.scale = Vec3::splat(scale);

        let alpha = ring.start_alpha * (1.0 - progress);
        let base = sprite.color.to_srgba();
        sprite.color = Color::srgba(base.red, base.green, base.blue, alpha);

        if ring.timer.just_finished() {
            cmd.entity(entity).try_despawn();
        }
    }
}

pub fn trail_ghost_update(
    mut cmd: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut Sprite, &mut TrailGhost)>,
) {
    for (entity, mut sprite, mut ghost) in q.iter_mut() {
        ghost.timer.tick(time.delta());
        let progress = ghost.timer.fraction().clamp(0.0, 1.0);
        let alpha = ghost.start_alpha * (1.0 - progress);
        let base = sprite.color.to_srgba();
        sprite.color = Color::srgba(base.red, base.green, base.blue, alpha);

        if ghost.timer.just_finished() {
            cmd.entity(entity).try_despawn();
        }
    }
}

pub fn hit_flash_update(
    mut cmd: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut Transform, &mut Sprite, &mut HitFlash)>,
) {
    for (entity, mut transform, mut sprite, mut flash) in q.iter_mut() {
        flash.timer.tick(time.delta());
        let progress = flash.timer.fraction().clamp(0.0, 1.0);
        let alpha = flash.start_alpha * (1.0 - progress);

        // Slight pop/shrink for that "impact" feel.
        let pop = 1.0 + (1.0 - progress) * 0.12;
        transform.scale = flash.base_scale * pop;

        let base = sprite.color.to_srgba();
        sprite.color = Color::srgba(base.red, base.green, base.blue, alpha);

        if flash.timer.just_finished() {
            cmd.entity(entity).try_despawn();
        }
    }
}

/// Spawn burst of particles when a power-up is collected
pub fn spawn_pickup_particles(cmd: &mut Commands, pos: Vec3, color: Color, count: usize) {
    let base = color.to_srgba();
    for i in 0..count {
        let angle = (i as f32 / count as f32) * std::f32::consts::TAU;
        let speed = 120.0 + (i as f32 % 3.0) * 40.0;
        let velocity = Vec2::new(angle.cos(), angle.sin()) * speed;

        cmd.spawn((
            Sprite {
                color: Color::srgba(base.red, base.green, base.blue, 0.9),
                custom_size: Some(Vec2::splat(6.0)),
                ..default()
            },
            Transform::from_translation(pos + Vec3::new(0.0, 0.0, 0.5)),
            PickupParticle {
                timer: Timer::from_seconds(0.4, TimerMode::Once),
                velocity,
                start_alpha: 0.9,
            },
            GameEntity,
        ));
    }
}

pub fn pickup_particle_update(
    mut cmd: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut Transform, &mut Sprite, &mut PickupParticle)>,
) {
    let dt = time.delta_secs();
    for (entity, mut transform, mut sprite, mut particle) in q.iter_mut() {
        particle.timer.tick(time.delta());
        let progress = particle.timer.fraction().clamp(0.0, 1.0);

        // Move outward with deceleration
        let decel = 1.0 - progress * 0.7;
        transform.translation.x += particle.velocity.x * dt * decel;
        transform.translation.y += particle.velocity.y * dt * decel;

        // Fade out and shrink
        let alpha = particle.start_alpha * (1.0 - progress);
        let base = sprite.color.to_srgba();
        sprite.color = Color::srgba(base.red, base.green, base.blue, alpha);
        sprite.custom_size = Some(Vec2::splat(6.0 * (1.0 - progress * 0.5)));

        if particle.timer.just_finished() {
            cmd.entity(entity).try_despawn();
        }
    }
}

// === Global ===

#[allow(clippy::too_many_arguments)]
pub fn game_restart(
    mut cmd: Commands,
    kb: Res<ButtonInput<KeyCode>>,
    mut state: GameStateMut,
    mut timers: ResMut<GameTimers>,
    game_state: Res<State<GameState>>,
    mut music: MusicResources,
    spawn: SpawnContext,
    cleanup: RestartCleanup,
    mut flash: ResMut<ScreenFlash>,
    mut energy: ResMut<PlayerEnergy>,
    mut drop_queue: ResMut<PowerUpDropQueue>,
    mut powerup_cd: ResMut<PowerUpSpawnCooldown>,
    mut intermission: ResMut<WaveIntermission>,
) {
    if kb.just_pressed(KeyCode::KeyR) {
        for e in cleanup.all() {
            cmd.entity(e).try_despawn();
        }
        for e in cleanup.player.iter() {
            cmd.entity(e).try_despawn();
        }
        state.data.reset();
        state.powerups.reset();
        energy.reset();
        drop_queue.0.clear();
        powerup_cd.0 = Timer::from_seconds(0.0, TimerMode::Once);
        intermission.cancel();
        timers.spiral = Timer::from_seconds(state.data.spiral_interval(), TimerMode::Once);
        timers.dive = Timer::from_seconds(state.data.dive_interval(), TimerMode::Once);
        timers.enemy_shoot = Timer::from_seconds(state.data.enemy_shoot_rate(), TimerMode::Once);
        timers.shoot = Timer::from_seconds(0.0, TimerMode::Once);
        spawn_player(&mut cmd, spawn.screen.half_height, &spawn.sprites, false);
        spawn_fading_text(&mut cmd, &spawn.fonts, "", 1.2, Color::NONE, true);
        super::ui::spawn_wave_transition_fx(&mut cmd, &spawn.fonts, spawn.screen.as_ref(), 1, 1.2);
        flash.trigger(0.22, Color::srgba(0.35, 0.55, 1.0, 0.25));

        spawn_life_icons(&mut cmd, &spawn.sprites, &spawn.screen, state.data.lives);
        spawn_score_digits(
            &mut cmd,
            &spawn.sprites,
            &spawn.screen,
            ScoreType::Score,
            state.data.score,
        );
        spawn_score_digits(
            &mut cmd,
            &spawn.sprites,
            &spawn.screen,
            ScoreType::HighScore,
            state.data.high_score,
        );
        spawn_wave_digits(&mut cmd, &spawn.sprites, &spawn.screen, state.data.wave);
        spawn_energy_bar(&mut cmd, spawn.screen.as_ref(), energy.as_ref());

        if music.enabled.0 {
            request_subsong(&mut music.fade, 2);
        }
        if *game_state.get() == GameState::GameOver {
            state.next_state.set(GameState::Playing);
        }
    }
}

pub fn game_quit(
    mut cmd: Commands,
    kb: Res<ButtonInput<KeyCode>>,
    fonts: Res<FontAssets>,
    mut quit_state: ResMut<QuitConfirmation>,
    quit_ui: Query<Entity, With<QuitConfirmUi>>,
    mut exit: MessageWriter<AppExit>,
) {
    if quit_state.showing {
        // Handle Y/N while confirmation is showing
        if kb.just_pressed(KeyCode::KeyY) {
            exit.write(AppExit::Success);
        }
        if kb.just_pressed(KeyCode::KeyN) || kb.just_pressed(KeyCode::Escape) {
            quit_state.showing = false;
            for e in quit_ui.iter() {
                cmd.entity(e).try_despawn();
            }
        }
    } else if kb.just_pressed(KeyCode::KeyQ) {
        // Show quit confirmation
        quit_state.showing = true;

        // Spawn confirmation UI
        cmd.spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(40.0),
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(20.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.85)),
            QuitConfirmUi,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("QUIT  GAME"),
                TextFont {
                    font: fonts.arcade.clone(),
                    font_size: 48.0,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.3, 0.3)),
                Node {
                    margin: UiRect::top(Val::Px(20.0)),
                    ..default()
                },
            ));
            parent.spawn((
                Text::new("Are  you  sure     Y  N"),
                TextFont {
                    font: fonts.arcade.clone(),
                    font_size: 32.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                Node {
                    margin: UiRect::bottom(Val::Px(20.0)),
                    ..default()
                },
            ));
        });
    }
}

pub fn music_toggle(kb: Res<ButtonInput<KeyCode>>, mut q: Query<&mut Ym2149Playback>) {
    if kb.just_pressed(KeyCode::KeyM)
        && let Ok(mut p) = q.single_mut()
    {
        if p.state == bevy_ym2149::PlaybackState::Playing {
            p.pause();
        } else {
            p.play();
        }
    }
}

// === Music Crossfade ===

pub fn request_subsong(fade: &mut MusicFade, subsong: usize) {
    fade.target_subsong = Some(subsong);
    fade.phase = FadePhase::FadeOut;
    fade.timer = 0.0;
}

pub fn music_crossfade(
    time: Res<Time>,
    mut fade: ResMut<MusicFade>,
    mut q: Query<&mut Ym2149Playback>,
) {
    if fade.phase == FadePhase::Idle {
        return;
    }
    let Ok(mut p) = q.single_mut() else { return };

    if let Some(target) = fade.target_subsong
        && p.current_subsong() == target
        && fade.phase == FadePhase::FadeOut
        && fade.timer == 0.0
    {
        fade.phase = FadePhase::Idle;
        fade.target_subsong = None;
        return;
    }

    fade.timer += time.delta_secs();
    let progress = (fade.timer / FADE_DURATION).min(1.0);

    match fade.phase {
        FadePhase::FadeOut => {
            p.set_volume(1.0 - progress);
            if progress >= 1.0 {
                if let Some(subsong) = fade.target_subsong {
                    p.set_subsong(subsong);
                    p.play();
                }
                fade.phase = FadePhase::FadeIn;
                fade.timer = 0.0;
            }
        }
        FadePhase::FadeIn => {
            p.set_volume(progress);
            if progress >= 1.0 {
                fade.phase = FadePhase::Idle;
                fade.target_subsong = None;
            }
        }
        FadePhase::Idle => {}
    }
}

// === Game Over ===

pub fn gameover_enemy_movement(
    time: Res<Time>,
    mut q: Query<(&mut Transform, &mut GameOverEnemy)>,
    screen: Res<ScreenSize>,
) {
    let (hw, hh) = (screen.half_width - 40.0, screen.half_height - 40.0);
    let t = time.elapsed_secs();
    let dt = time.delta_secs();

    for (mut tr, mut ge) in q.iter_mut() {
        if !ge.started {
            ge.delay -= dt;
            if ge.delay <= 0.0 {
                ge.started = true;
                ge.phase = t;
            }
            continue;
        }
        let local_t = (t - ge.phase) * ge.frequency;
        let blend = ((t - ge.phase) / 2.0).min(1.0);
        let target_x = (local_t + ge.amplitude).sin() * hw;
        let target_y = (local_t * 0.7 + ge.amplitude * 0.5).sin() * hh;
        tr.translation.x = ge.base_pos.x + (target_x - ge.base_pos.x) * blend;
        tr.translation.y = ge.base_pos.y + (target_y - ge.base_pos.y) * blend;
    }
}

pub fn gameover_ui_animation(time: Res<Time>, mut q: Query<(&mut Node, &GameOverUi)>) {
    let t = time.elapsed_secs();
    for (mut node, ui) in q.iter_mut() {
        let offset = (t * 1.5).sin() * 1.5;
        node.top = Val::Percent(ui.base_top + offset);
    }
}

// === Name Entry ===

#[allow(clippy::too_many_arguments)]
pub fn enter_name_entry(
    mut cmd: Commands,
    gd: Res<GameData>,
    spawn: SpawnContext,
    mut name_state: ResMut<NameEntryState>,
    mut cleanup: TransitionCleanup,
    mut intermission: ResMut<WaveIntermission>,
    mut combo: ResMut<ComboTracker>,
    mut uiq: Query<(&mut Visibility, &UiMarker)>,
) {
    let mut rng = SmallRng::from_os_rng();
    intermission.cancel();
    combo.count = 0;
    combo.timer = 0.0;
    for (mut v, m) in uiq.iter_mut() {
        if *m == UiMarker::Combo {
            *v = Visibility::Hidden;
        }
    }

    cleanup_game_entities(
        &mut cmd,
        &mut cleanup.powerups,
        cleanup.game_entities.all(),
        std::iter::empty(),
    );

    // Also clean up score popups and shield bubbles
    for e in cleanup.effects.popups.iter() {
        cmd.entity(e).try_despawn();
    }
    for e in cleanup.effects.bubbles.iter() {
        cmd.entity(e).try_despawn();
    }
    for e in cleanup.bosses.iter() {
        cmd.entity(e).try_despawn();
    }
    for e in cleanup.boss_ui.iter() {
        cmd.entity(e).try_despawn();
    }

    let enemies: Vec<_> = cleanup.enemies.iter().collect();
    setup_enemies_sine_wave(&mut cmd, &enemies, &mut rng);

    *name_state = NameEntryState::default();
    super::ui::spawn_name_entry_ui(&mut cmd, &spawn.fonts, gd.score, gd.wave);
}

pub fn name_entry_input(
    kb: Res<ButtonInput<KeyCode>>,
    mut name_state: ResMut<NameEntryState>,
    mut ns: ResMut<NextState<GameState>>,
    mut scores: ResMut<HighScoreList>,
    mut new_hs_index: ResMut<NewHighScoreIndex>,
    gd: Res<GameData>,
    mut q: Query<(&mut Text, &mut TextColor, &NameEntryChar)>,
) {
    let pos = name_state.position;

    // Character change
    if kb.just_pressed(KeyCode::ArrowUp) {
        let c = &mut name_state.chars[pos];
        *c = match *c {
            'Z' => 'A',
            _ => ((*c as u8) + 1) as char,
        };
    }
    if kb.just_pressed(KeyCode::ArrowDown) {
        let c = &mut name_state.chars[pos];
        *c = match *c {
            'A' => 'Z',
            _ => ((*c as u8) - 1) as char,
        };
    }

    // Position change
    if kb.just_pressed(KeyCode::ArrowRight) && name_state.position < 2 {
        name_state.position += 1;
    }
    if kb.just_pressed(KeyCode::ArrowLeft) && name_state.position > 0 {
        name_state.position -= 1;
    }

    // Confirm
    if kb.just_pressed(KeyCode::Enter) {
        let name: String = name_state.chars.iter().collect();
        let index = scores.add_score(name, gd.score, gd.wave);
        new_hs_index.0 = Some(index);
        ns.set(GameState::HighScores);
        return;
    }

    // Update display
    for (mut text, mut color, char_marker) in q.iter_mut() {
        text.0 = name_state.chars[char_marker.index].to_string();
        color.0 = if char_marker.index == name_state.position {
            Color::srgb(1.0, 1.0, 0.2)
        } else {
            Color::srgb(0.5, 0.5, 0.5)
        };
    }
}

pub fn name_entry_blink(
    time: Res<Time>,
    name_state: Res<NameEntryState>,
    mut q: Query<(&mut TextColor, &NameEntryChar)>,
) {
    let blink = (time.elapsed_secs() * 4.0).sin() > 0.0;
    for (mut color, char_marker) in q.iter_mut() {
        if char_marker.index == name_state.position {
            color.0 = if blink {
                Color::srgb(1.0, 1.0, 0.2)
            } else {
                Color::srgb(0.8, 0.6, 0.0)
            };
        }
    }
}

pub fn exit_name_entry(
    mut cmd: Commands,
    q: Query<Entity, With<NameEntryUi>>,
    enemies: Query<Entity, With<GameOverEnemy>>,
) {
    for e in q.iter() {
        cmd.entity(e).try_despawn();
    }
    // Clean up flying enemies when leaving name entry
    for e in enemies.iter() {
        cmd.entity(e).try_despawn();
    }
}

// === High Scores Screen ===

pub fn enter_high_scores(mut cmd: Commands, fonts: Res<FontAssets>, scores: Res<HighScoreList>) {
    super::ui::spawn_high_scores_ui(&mut cmd, &fonts, &scores);
    cmd.insert_resource(ScreenCycleTimer::default());
}

pub fn high_scores_input(
    kb: Res<ButtonInput<KeyCode>>,
    mut ns: ResMut<NextState<GameState>>,
    mut attract: ResMut<AttractMode>,
) {
    if kb.just_pressed(KeyCode::Enter) {
        attract.active = false; // Manual exit disables attract mode
        ns.set(GameState::TitleScreen);
    }
}

pub fn high_scores_auto_return(
    time: Res<Time>,
    attract: Res<AttractMode>,
    mut fade: ResMut<ScreenFade>,
    mut timer: ResMut<ScreenCycleTimer>,
    mut ns: ResMut<NextState<GameState>>,
) {
    if !attract.active {
        return;
    }

    timer.0.tick(time.delta());

    // Start fade-out 0.5s before timer ends
    let remaining = timer.0.remaining_secs();
    if remaining <= ScreenFade::FADE_DURATION && !fade.fading_out && fade.alpha > 0.0 {
        fade.start_fade_out();
    }

    if timer.0.just_finished() {
        ns.set(GameState::PowerUpsScreen);
    }
}

pub fn high_scores_fade(
    time: Res<Time>,
    fade: Res<ScreenFade>,
    new_hs_index: Res<NewHighScoreIndex>,
    mut q: Query<(&mut TextColor, Option<&HighScoreRow>), With<HighScoresUi>>,
) {
    let alpha = fade.alpha;
    let highlight_idx = new_hs_index.0;
    let blink = (time.elapsed_secs() * 6.0).sin() > 0.0;

    for (mut color, row) in q.iter_mut() {
        let base = color.0.to_srgba();

        // Apply highlight blink if this is the new high score row
        let is_highlighted = row.is_some_and(|r| Some(r.0) == highlight_idx);
        let row_alpha = if is_highlighted && !blink {
            alpha * 0.3
        } else {
            alpha
        };

        color.0 = Color::srgba(base.red, base.green, base.blue, row_alpha);
    }
}

pub fn exit_high_scores(
    mut cmd: Commands,
    mut new_hs_index: ResMut<NewHighScoreIndex>,
    q: Query<Entity, With<HighScoresUi>>,
) {
    new_hs_index.0 = None;
    for e in q.iter() {
        cmd.entity(e).try_despawn();
    }
}

// === Power-ups Screen ===

pub fn enter_powerups_screen(
    mut cmd: Commands,
    fonts: Res<FontAssets>,
    sprites: Res<SpriteAssets>,
    screen: Res<ScreenSize>,
) {
    super::ui::spawn_powerups_ui(&mut cmd, &fonts, &sprites, &screen);
    cmd.insert_resource(ScreenCycleTimer::default());
}

pub fn powerups_screen_input(
    kb: Res<ButtonInput<KeyCode>>,
    mut ns: ResMut<NextState<GameState>>,
    mut attract: ResMut<AttractMode>,
) {
    if kb.just_pressed(KeyCode::Enter) {
        attract.active = false;
        ns.set(GameState::TitleScreen);
    }
}

pub fn powerups_screen_auto_cycle(
    time: Res<Time>,
    attract: Res<AttractMode>,
    mut fade: ResMut<ScreenFade>,
    mut timer: ResMut<ScreenCycleTimer>,
    mut ns: ResMut<NextState<GameState>>,
) {
    if !attract.active {
        return;
    }

    timer.0.tick(time.delta());

    let remaining = timer.0.remaining_secs();
    if remaining <= ScreenFade::FADE_DURATION && !fade.fading_out && fade.alpha > 0.0 {
        fade.start_fade_out();
    }

    if timer.0.just_finished() {
        ns.set(GameState::EnemyScoresScreen);
    }
}

pub fn powerups_screen_fade(fade: Res<ScreenFade>, mut q: Query<&mut TextColor, With<PowerUpsUi>>) {
    let alpha = fade.alpha;
    for mut color in q.iter_mut() {
        let base = color.0.to_srgba();
        color.0 = Color::srgba(base.red, base.green, base.blue, alpha);
    }
}

pub fn exit_powerups_screen(mut cmd: Commands, q: Query<Entity, With<PowerUpsUi>>) {
    for e in q.iter() {
        cmd.entity(e).try_despawn();
    }
}

// === Enemy Scores Screen ===

pub fn enter_enemy_scores_screen(
    mut cmd: Commands,
    fonts: Res<FontAssets>,
    sprites: Res<SpriteAssets>,
    screen: Res<ScreenSize>,
) {
    super::ui::spawn_enemy_scores_ui(&mut cmd, &fonts, &sprites, &screen);
    cmd.insert_resource(ScreenCycleTimer::default());
}

pub fn enemy_scores_screen_input(
    kb: Res<ButtonInput<KeyCode>>,
    mut ns: ResMut<NextState<GameState>>,
    mut attract: ResMut<AttractMode>,
) {
    if kb.just_pressed(KeyCode::Enter) {
        attract.active = false;
        ns.set(GameState::TitleScreen);
    }
}

pub fn enemy_scores_screen_auto_cycle(
    time: Res<Time>,
    attract: Res<AttractMode>,
    mut fade: ResMut<ScreenFade>,
    mut timer: ResMut<ScreenCycleTimer>,
    mut ns: ResMut<NextState<GameState>>,
) {
    if !attract.active {
        return;
    }

    timer.0.tick(time.delta());

    let remaining = timer.0.remaining_secs();
    if remaining <= ScreenFade::FADE_DURATION && !fade.fading_out && fade.alpha > 0.0 {
        fade.start_fade_out();
    }

    if timer.0.just_finished() {
        ns.set(GameState::TitleScreen);
    }
}

pub fn enemy_scores_screen_fade(
    fade: Res<ScreenFade>,
    mut q: Query<&mut TextColor, With<EnemyScoresUi>>,
) {
    let alpha = fade.alpha;
    for mut color in q.iter_mut() {
        let base = color.0.to_srgba();
        color.0 = Color::srgba(base.red, base.green, base.blue, alpha);
    }
}

/// Animate wavy text with per-line phase offset
pub fn wavy_text_animation(time: Res<Time>, mut q: Query<(&mut Node, &WavyText)>) {
    let t = time.elapsed_secs();
    for (mut node, wavy) in q.iter_mut() {
        // Each line has a phase offset based on its index
        let phase = wavy.line_index as f32 * 0.4;
        let offset = (t * 1.5 + phase).sin() * 0.3; // 0.3% amplitude, 1.5 Hz
        node.top = Val::Percent(wavy.base_top + offset);
    }
}

/// Animate wavy sprites (2D entities) with per-line phase offset
pub fn wavy_sprite_animation(time: Res<Time>, mut q: Query<(&mut Transform, &WavySprite)>) {
    let t = time.elapsed_secs();
    for (mut transform, wavy) in q.iter_mut() {
        let phase = wavy.line_index as f32 * 0.4;
        let offset = (t * 1.5 + phase).sin() * 4.0; // 4 pixels amplitude
        transform.translation.y = wavy.base_y + offset;
    }
}

pub fn exit_enemy_scores_screen(
    mut cmd: Commands,
    q: Query<Entity, With<EnemyScoresUi>>,
    enemies: Query<Entity, With<GameOverEnemy>>,
) {
    for e in q.iter() {
        cmd.entity(e).try_despawn();
    }
    // Despawn flying enemies when returning to title
    for e in enemies.iter() {
        cmd.entity(e).try_despawn();
    }
}

// === Power-ups ===

pub fn powerup_movement(
    mut cmd: Commands,
    time: Res<Time>,
    screen: Res<ScreenSize>,
    mut q: Query<(Entity, &mut Transform), With<PowerUp>>,
) {
    let bottom = -screen.half_height - 20.0;
    for (entity, mut t) in q.iter_mut() {
        t.translation.y -= POWERUP_SPEED * time.delta_secs();
        // Gentle floating motion
        t.translation.x += (time.elapsed_secs() * 2.0 + t.translation.y * 0.1).sin() * 0.5;
        if t.translation.y < bottom {
            cmd.entity(entity).try_despawn();
        }
    }
}

pub fn powerup_collection(
    mut cmd: Commands,
    powerups: Query<(Entity, &Transform, &PowerUp)>,
    player: Query<&Transform, With<Player>>,
    mut collected: MessageWriter<PowerUpCollectedMsg>,
    mut sfx: MessageWriter<PlaySfxMsg>,
) {
    let Ok(pt) = player.single() else { return };
    let pp = pt.translation.truncate();

    for (entity, t, powerup) in powerups.iter() {
        if t.translation.truncate().distance(pp) < POWERUP_SIZE.x / 2.0 + PLAYER_SIZE.x / 2.0 {
            cmd.entity(entity).try_despawn();
            collected.write(PowerUpCollectedMsg(powerup.kind, t.translation));
            sfx.write(PlaySfxMsg(SfxType::PowerUp));
        }
    }
}

pub fn handle_powerup_collected(
    mut cmd: Commands,
    mut events: MessageReader<PowerUpCollectedMsg>,
    mut gd: ResMut<GameData>,
    fonts: Res<FontAssets>,
    mut extra: MessageWriter<ExtraLifeMsg>,
    mut ctx: PowerUpContext,
) {
    for ev in events.read() {
        let (kind, pos) = (ev.0, ev.1);

        let mut awarded_points = 0u32;
        let mut applied = false;

        match kind {
            PowerUpType::Heal => {
                applied = ctx.energy.heal(2);
                if !applied {
                    awarded_points = POWERUP_DUPLICATE_SCORE;
                }
            }
            PowerUpType::RapidFire => {
                if ctx.state.rapid_fire {
                    awarded_points = POWERUP_DUPLICATE_SCORE;
                } else {
                    ctx.state.rapid_fire = true;
                    applied = true;
                }
            }
            PowerUpType::TripleShot => {
                if ctx.state.triple_shot {
                    awarded_points = POWERUP_DUPLICATE_SCORE;
                } else {
                    ctx.state.triple_shot = true;
                    applied = true;
                }
            }
            PowerUpType::SpeedBoost => {
                if ctx.state.speed_boost {
                    awarded_points = POWERUP_DUPLICATE_SCORE;
                } else {
                    ctx.state.speed_boost = true;
                    applied = true;
                }
            }
            PowerUpType::PowerShot => {
                if ctx.state.power_shot {
                    awarded_points = POWERUP_DUPLICATE_SCORE;
                } else {
                    ctx.state.power_shot = true;
                    applied = true;
                }
            }
        }

        // Get flash color based on power-up type
        let flash_color = match kind {
            PowerUpType::Heal => Color::srgba(0.35, 1.0, 0.35, 0.35),
            PowerUpType::RapidFire => Color::srgba(1.0, 0.4, 0.4, 0.4),
            PowerUpType::TripleShot => Color::srgba(1.0, 0.35, 0.95, 0.35),
            PowerUpType::SpeedBoost => Color::srgba(0.4, 0.6, 1.0, 0.4),
            PowerUpType::PowerShot => Color::srgba(1.0, 1.0, 0.4, 0.4),
        };

        if awarded_points > 0 {
            award_points(&mut gd, awarded_points, &mut extra);
            spawn_score_popup(&mut cmd, &fonts, pos, awarded_points, 1);
        }

        // Trigger screen flash
        let duration = if applied { 0.18 } else { 0.10 };
        ctx.flash.trigger(duration, flash_color);

        // Spawn pickup particles (brighter version of flash color)
        let particle_color = match kind {
            PowerUpType::Heal => Color::srgb(0.4, 1.0, 0.5),
            PowerUpType::RapidFire => Color::srgb(1.0, 0.5, 0.4),
            PowerUpType::TripleShot => Color::srgb(1.0, 0.45, 1.0),
            PowerUpType::SpeedBoost => Color::srgb(0.5, 0.7, 1.0),
            PowerUpType::PowerShot => Color::srgb(1.0, 1.0, 0.5),
        };
        let particle_count = if applied { 12 } else { 6 };
        spawn_pickup_particles(&mut cmd, pos, particle_color, particle_count);

        // Spawn side boosters if we have any power-up and don't have them yet
        if ctx.state.has_any()
            && ctx.side_boosters.is_empty()
            && let Ok(player_id) = ctx.player.single()
        {
            spawn_side_boosters(&mut cmd, player_id, &ctx.sprites);
        }
    }
}

/// Update screen flash overlay
pub fn screen_flash_system(
    time: Res<Time>,
    screen: Res<ScreenSize>,
    mut flash: ResMut<ScreenFlash>,
    mut overlays: Query<(&mut Sprite, &mut Visibility), With<ScreenFlashOverlay>>,
) {
    let mut active = false;
    if flash.timer > 0.0 {
        flash.timer -= time.delta_secs();
        active = flash.timer > 0.0;
    }

    // Fade out the overlay (single persistent entity; if multiples exist, update all).
    let base = flash.color.to_srgba();
    let alpha = (flash.strength() * base.alpha * 0.35).min(0.28);
    let size = Vec2::new(screen.width * 2.0, screen.height * 2.0);
    for (mut sprite, mut vis) in overlays.iter_mut() {
        sprite.custom_size = Some(size);
        if active {
            *vis = Visibility::Visible;
            sprite.color = Color::srgba(base.red, base.green, base.blue, alpha);
        } else {
            *vis = Visibility::Hidden;
            sprite.color = Color::srgba(base.red, base.green, base.blue, 0.0);
        }
    }
}

/// Handle invincibility timer and flicker effect
pub fn invincibility_system(
    mut cmd: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut Invincible, &mut Sprite), With<Player>>,
    bubbles: Query<Entity, With<ShieldBubble>>,
) {
    for (entity, mut invincible, mut sprite) in q.iter_mut() {
        invincible.timer.tick(time.delta());

        // Flicker effect - rapid alpha toggle
        let t = time.elapsed_secs();
        let flicker = (t * 15.0).sin() > 0.0; // Fast flicker
        sprite.color = if flicker {
            Color::srgba(1.0, 1.0, 1.0, 1.0)
        } else {
            Color::srgba(1.0, 1.0, 1.0, 0.3)
        };

        // Remove invincibility when timer is done
        if invincible.timer.is_finished() {
            sprite.color = Color::WHITE;
            cmd.entity(entity).remove::<Invincible>();

            // Also remove the shield bubble
            for bubble in bubbles.iter() {
                cmd.entity(bubble).try_despawn();
            }
        }
    }
}

/// Shield bubble follows player and pulses/fades based on invincibility timer
pub fn shield_bubble_system(
    time: Res<Time>,
    player_q: Query<(&Transform, &Invincible), With<Player>>,
    mut bubble_q: ShieldBubbleQuery,
) {
    let Ok((player_transform, invincible)) = player_q.single() else {
        return;
    };

    let remaining = invincible.timer.remaining_secs();
    let total = invincible.timer.duration().as_secs_f32();
    let progress = remaining / total; // 1.0 -> 0.0 as time passes

    let t = time.elapsed_secs();

    for (mut transform, mut sprite) in bubble_q.iter_mut() {
        // Follow player position
        transform.translation.x = player_transform.translation.x;
        transform.translation.y = player_transform.translation.y;

        // Pulsing size effect
        let pulse = 1.0 + (t * 8.0).sin() * 0.15;
        transform.scale = Vec3::splat(pulse);

        // Fade out as invincibility runs out (more visible at start, fades toward end)
        let base_alpha = 0.5 * progress; // Fades from 0.5 to 0 over time
        let flash = ((t * 6.0).sin() + 1.0) * 0.5; // 0.0 - 1.0 oscillation
        let alpha = base_alpha * (0.6 + flash * 0.4);

        sprite.color = Color::srgba(1.0, 0.9, 0.2, alpha);
    }
}

// === Visual Effects Systems ===

/// Apply screen shake to the game camera
pub fn screen_shake_system(
    time: Res<Time>,
    mut shake: ResMut<ScreenShake>,
    mut camera_q: Query<&mut Transform, With<GameCamera>>,
) {
    if shake.trauma <= 0.0 {
        // Reset camera position when not shaking
        for mut transform in camera_q.iter_mut() {
            transform.translation.x = 0.0;
            transform.translation.y = 0.0;
        }
        return;
    }

    // Decay trauma over time
    shake.trauma = (shake.trauma - shake.decay_rate * time.delta_secs()).max(0.0);

    // Use trauma^2 for smoother falloff
    let shake_amount = shake.trauma * shake.trauma * SHAKE_INTENSITY;

    // Perlin-like noise using time
    let t = time.elapsed_secs();
    let offset_x = (t * 30.0).sin() * shake_amount;
    let offset_y = (t * 40.0).cos() * shake_amount;

    for mut transform in camera_q.iter_mut() {
        transform.translation.x = offset_x;
        transform.translation.y = offset_y;
    }
}

/// Spawn a score popup at a position with optional combo multiplier
pub fn spawn_score_popup(
    cmd: &mut Commands,
    fonts: &FontAssets,
    pos: Vec3,
    points: u32,
    combo: u32,
) {
    let color = match points {
        100 => Color::srgb(1.0, 0.3, 0.3), // Red for high value
        50 => Color::srgb(1.0, 1.0, 0.3),  // Yellow for mid
        _ => Color::srgb(0.3, 1.0, 0.3),   // Green for low
    };

    // Show combo if > 1
    let text = if combo > 1 {
        format!("{points} x{combo}")
    } else {
        format!("{points}")
    };

    cmd.spawn((
        Text2d::new(text),
        TextFont {
            font: fonts.arcade.clone(),
            font_size: 24.0,
            ..default()
        },
        TextColor(color),
        Transform::from_translation(pos + Vec3::new(0.0, 20.0, 10.0)),
        ScorePopup {
            timer: Timer::from_seconds(SCORE_POPUP_DURATION, TimerMode::Once),
            start_y: pos.y + 20.0,
        },
        GameEntity,
    ));
}

/// Animate score popups (rise and fade)
pub fn score_popup_system(
    mut cmd: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut Transform, &mut TextColor, &mut ScorePopup)>,
) {
    for (entity, mut transform, mut color, mut popup) in q.iter_mut() {
        popup.timer.tick(time.delta());

        let progress = popup.timer.fraction();

        // Rise up
        transform.translation.y = popup.start_y + progress * SCORE_POPUP_RISE;

        // Fade out
        let alpha = 1.0 - progress;
        let base = color.0.to_srgba();
        color.0 = Color::srgba(base.red, base.green, base.blue, alpha);

        // Remove when done
        if popup.timer.just_finished() {
            cmd.entity(entity).try_despawn();
        }
    }
}

/// Parallax starfield with multiple layers
pub fn parallax_starfield(
    time: Res<Time>,
    screen: Res<ScreenSize>,
    player: Query<&Transform, With<Player>>,
    mut q: ParallaxStarQuery<'_, '_>,
) {
    let dt = time.delta_secs();
    let hh = screen.half_height;
    let player_x = player.iter().next().map(|t| t.translation.x).unwrap_or(0.0);

    for (mut transform, mut sprite, star, layer, anchor, twinkle) in q.iter_mut() {
        // Different speed multipliers per layer
        let speed_mult = match layer.0 {
            0 => 0.3, // Far/slow
            1 => 0.6, // Mid
            _ => 1.0, // Near/fast (default Star behavior)
        };

        transform.translation.y -= star.speed * speed_mult * dt;

        // Wrap around
        if transform.translation.y < -hh - 10.0 {
            transform.translation.y = hh + 10.0;
        }

        // Subtle horizontal parallax relative to player movement.
        let parallax = player_x * 0.02 * speed_mult;
        transform.translation.x = anchor.base_x - parallax;

        // Twinkle (brightness only; keeps tint).
        let t = time.elapsed_secs();
        let tw = (t * twinkle.speed + twinkle.phase).sin() * 0.5 + 0.5;
        let brightness = (twinkle.base + tw * twinkle.amplitude).clamp(0.0, 1.0);
        sprite.color = Color::srgb(
            brightness * twinkle.tint.x,
            brightness * twinkle.tint.y,
            brightness * twinkle.tint.z,
        );
    }
}

/// Slow-moving nebula clouds for deep parallax effect
pub fn nebula_movement(
    time: Res<Time>,
    screen: Res<ScreenSize>,
    player: Query<&Transform, With<Player>>,
    mut q: Query<
        (
            &mut Transform,
            &mut Sprite,
            &Nebula,
            &ParallaxAnchor,
            &NebulaPulse,
        ),
        Without<Player>,
    >,
) {
    let dt = time.delta_secs();
    let hh = screen.half_height;
    let player_x = player.iter().next().map(|t| t.translation.x).unwrap_or(0.0);
    let t = time.elapsed_secs();

    for (mut transform, mut sprite, nebula, anchor, pulse) in q.iter_mut() {
        transform.translation.y -= nebula.speed * dt;

        // Wrap around (with larger margin for big sprites)
        if transform.translation.y < -hh - 200.0 {
            transform.translation.y = hh + 200.0;
        }

        // Slow drift + player parallax.
        let drift = (t * pulse.speed + pulse.phase).sin() * 22.0;
        transform.translation.x = anchor.base_x - player_x * 0.06 + drift;

        // Gentle alpha pulse to add depth.
        let p = (t * pulse.speed + pulse.phase).sin() * 0.5 + 0.5;
        let alpha = (pulse.base_alpha + (p - 0.5) * 2.0 * pulse.amplitude).clamp(0.05, 0.6);
        let base = sprite.color.to_srgba();
        sprite.color = Color::srgba(base.red, base.green, base.blue, alpha);
    }
}
