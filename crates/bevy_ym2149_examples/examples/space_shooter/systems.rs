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

type EnemyBulletQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static mut Transform), (With<EnemyBullet>, Without<PlayerBullet>)>;
type FormationEnemyQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static Transform), (With<Enemy>, Without<DivingEnemy>)>;
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
    pub player_bullets: Query<'w, 's, (Entity, &'static Transform), With<PlayerBullet>>,
    pub enemy_bullets: Query<'w, 's, (Entity, &'static Transform), With<EnemyBullet>>,
    pub enemies: Query<'w, 's, (Entity, &'static Transform, &'static Enemy)>,
    pub player: Query<'w, 's, &'static Transform, With<Player>>,
    pub divers: Query<'w, 's, (Entity, &'static Transform), With<DivingEnemy>>,
}

/// Entities to clean up on game restart
#[derive(SystemParam)]
pub struct RestartCleanup<'w, 's> {
    pub enemies: Query<'w, 's, Entity, With<Enemy>>,
    pub bullets: AllBulletsQuery<'w, 's>,
    pub player: Query<'w, 's, Entity, With<Player>>,
    pub ui: GameplayCleanupQuery<'w, 's>,
}

impl RestartCleanup<'_, '_> {
    pub fn all(&self) -> impl Iterator<Item = Entity> + '_ {
        self.enemies
            .iter()
            .chain(self.bullets.iter())
            .chain(self.ui.iter())
    }
}

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

pub fn title_input(kb: Res<ButtonInput<KeyCode>>, mut ns: ResMut<NextState<GameState>>) {
    if kb.just_pressed(KeyCode::Enter) {
        ns.set(GameState::Playing);
    }
    if kb.just_pressed(KeyCode::KeyH) {
        ns.set(GameState::HighScores);
    }
    if kb.just_pressed(KeyCode::Escape) {
        std::process::exit(0);
    }
}

pub fn title_anim(
    time: Res<Time>,
    fade: Res<ScreenFade>,
    mut title_q: Query<(&mut TextColor, &UiMarker)>,
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

pub fn enter_playing(
    mut cmd: Commands,
    mut gd: ResMut<GameData>,
    mut fade: ResMut<MusicFade>,
    mut powerups: ResMut<PowerUpState>,
    music_enabled: Res<MusicEnabled>,
    spawn: SpawnContext,
    mut uiq: Query<(&mut Visibility, &UiMarker)>,
    old_enemies: Query<Entity, With<GameOverEnemy>>,
) {
    // Clean up any leftover enemies from game over
    for e in old_enemies.iter() {
        cmd.entity(e).try_despawn();
    }

    if music_enabled.0 {
        request_subsong(&mut fade, 2);
    }
    gd.reset();
    powerups.reset();
    spawn_player(&mut cmd, spawn.screen.half_height, &spawn.sprites, false);
    spawn_fading_text(
        &mut cmd,
        &spawn.fonts,
        "WAVE 1",
        2.0,
        Color::srgba(1.0, 1.0, 0.2, 1.0),
        true,
    );

    spawn_life_icons(&mut cmd, &spawn.sprites, &spawn.screen, gd.lives);
    spawn_score_digits(
        &mut cmd,
        &spawn.sprites,
        &spawn.screen,
        ScoreType::Score,
        gd.score,
    );
    spawn_score_digits(
        &mut cmd,
        &spawn.sprites,
        &spawn.screen,
        ScoreType::HighScore,
        gd.high_score,
    );
    spawn_wave_digits(&mut cmd, &spawn.sprites, &spawn.screen, gd.wave);

    for (mut v, _) in uiq.iter_mut() {
        *v = Visibility::Hidden;
    }
}

pub fn enter_gameover(
    mut cmd: Commands,
    mut fade: ResMut<MusicFade>,
    mut powerups: ResMut<PowerUpState>,
    music_enabled: Res<MusicEnabled>,
    mut uiq: Query<(Entity, &mut Visibility, &UiMarker)>,
    eq: Query<(Entity, &Transform), With<Enemy>>,
    cleanup: GameEndCleanup,
    mut rng: Local<Option<SmallRng>>,
) {
    let rng = rng.get_or_insert_with(SmallRng::from_os_rng);
    if music_enabled.0 {
        request_subsong(&mut fade, 3);
    }

    cleanup_game_entities(&mut cmd, &mut powerups, cleanup.all(), std::iter::empty());

    cmd.insert_resource(ScreenCycleTimer::default());

    for (entity, mut v, m) in uiq.iter_mut() {
        if *m == UiMarker::GameOver {
            *v = Visibility::Visible;
            cmd.entity(entity).insert(GameOverUi { base_top: 45.0 });
        }
    }

    let enemies: Vec<_> = eq.iter().collect();
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
    flash_overlays: Query<Entity, With<ScreenFlashOverlay>>,
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
    for e in flash_overlays.iter() {
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
                    SfxType::PowerUp => &s.explode, // Reuse explode for now
                },
                None,
                None,
            );
        }
    }
}

pub fn handle_wave_complete(
    mut cmd: Commands,
    mut events: MessageReader<WaveCompleteMsg>,
    mut gd: ResMut<GameData>,
    fonts: Res<FontAssets>,
) {
    for _ in events.read() {
        gd.wave += 1;
        gd.enemy_direction = 1.0;
        gd.dive_timer = Timer::from_seconds(gd.dive_interval(), TimerMode::Once);
        spawn_fading_text(
            &mut cmd,
            &fonts,
            &format!("WAVE {}", gd.wave),
            2.0,
            Color::srgba(1.0, 1.0, 0.2, 1.0),
            true,
        );
    }
}

pub fn handle_player_hit(
    mut cmd: Commands,
    mut events: MessageReader<PlayerHitMsg>,
    mut state: GameStateMut,
    player_q: PlayerQueries,
    spawn: SpawnContext,
    scores: Res<HighScoreList>,
    mut sfx: MessageWriter<PlaySfxMsg>,
) {
    for _ in events.read() {
        sfx.write(PlaySfxMsg(SfxType::Death));
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

        if state.data.lives == 0 {
            if scores.is_high_score(state.data.score) {
                state.next_state.set(GameState::NameEntry);
            } else {
                state.next_state.set(GameState::GameOver);
            }
        } else {
            // Respawn with invincibility
            spawn_player(&mut cmd, spawn.screen.half_height, &spawn.sprites, true);
        }
    }
}

pub fn handle_enemy_killed(
    mut events: MessageReader<EnemyKilledMsg>,
    mut gd: ResMut<GameData>,
    mut sfx: MessageWriter<PlaySfxMsg>,
    mut extra: MessageWriter<ExtraLifeMsg>,
) {
    for ev in events.read() {
        gd.score += ev.0;
        gd.high_score = gd.high_score.max(gd.score);
        sfx.write(PlaySfxMsg(SfxType::Explode));
        if gd.score >= gd.next_extra_life {
            gd.next_extra_life += EXTRA_LIFE_SCORE;
            extra.write(ExtraLifeMsg);
        }
    }
}

pub fn handle_extra_life(
    mut cmd: Commands,
    mut events: MessageReader<ExtraLifeMsg>,
    mut gd: ResMut<GameData>,
    fonts: Res<FontAssets>,
) {
    for _ in events.read() {
        gd.lives += 1;
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
                spawn_enemies(&mut cmd, &sprites);
            }
        }
    }
}

pub fn player_movement(
    kb: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut q: Query<(&mut Transform, &mut Sprite), With<Player>>,
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

pub fn player_shooting(
    mut cmd: Commands,
    kb: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut gd: ResMut<GameData>,
    pq: Query<&Transform, With<Player>>,
    ctx: ShootingContext,
    mut sfx: MessageWriter<PlaySfxMsg>,
) {
    gd.shoot_timer.tick(time.delta());
    if kb.pressed(KeyCode::Space)
        && gd.shoot_timer.is_finished()
        && let Ok(pt) = pq.single()
    {
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
            0.25
        };
        gd.shoot_timer = Timer::from_seconds(fire_rate, TimerMode::Once);
    }
}

pub fn bullet_movement(
    mut cmd: Commands,
    time: Res<Time>,
    screen: Res<ScreenSize>,
    mut player_bullets: Query<(Entity, &mut Transform), With<PlayerBullet>>,
    mut enemy_bullets: EnemyBulletQuery,
) {
    let (hw, hh) = (screen.half_width, screen.half_height);
    let dt = time.delta_secs();

    for (e, mut t) in player_bullets.iter_mut() {
        // Get direction from rotation (bullets are rotated with -angle_rad)
        let (_, angle) = t.rotation.to_axis_angle();
        let dir = Vec2::new(-angle.sin(), angle.cos());
        t.translation.x += dir.x * BULLET_SPEED * dt;
        t.translation.y += dir.y * BULLET_SPEED * dt;

        if t.translation.y > hh + 20.0 || t.translation.x.abs() > hw + 20.0 {
            cmd.entity(e).try_despawn();
        }
    }
    for (e, mut t) in enemy_bullets.iter_mut() {
        t.translation.y -= ENEMY_BULLET_SPEED * dt;
        if t.translation.y < -hh - 20.0 {
            cmd.entity(e).try_despawn();
        }
    }
}

pub fn enemy_formation_movement(
    time: Res<Time>,
    mut gd: ResMut<GameData>,
    mut q: Query<&mut Transform, (With<Enemy>, Without<DivingEnemy>)>,
    screen: Res<ScreenSize>,
) {
    let hw = screen.half_width - ENEMY_SIZE.x;
    let edge = q.iter().any(|t| {
        (t.translation.x > hw && gd.enemy_direction > 0.0)
            || (t.translation.x < -hw && gd.enemy_direction < 0.0)
    });
    if edge {
        gd.enemy_direction *= -1.0;
    }
    for mut t in q.iter_mut() {
        t.translation.x += gd.enemy_direction * 50.0 * time.delta_secs();
        if edge {
            t.translation.y -= 20.0;
        }
    }
}

pub fn enemy_shooting(
    mut cmd: Commands,
    time: Res<Time>,
    mut gd: ResMut<GameData>,
    sprites: Res<SpriteAssets>,
    q: Query<&Transform, With<Enemy>>,
    mut rng: Local<Option<SmallRng>>,
) {
    let rng = rng.get_or_insert_with(SmallRng::from_os_rng);
    gd.enemy_shoot_timer.tick(time.delta());
    if gd.enemy_shoot_timer.is_finished() {
        let enemies: Vec<_> = q.iter().collect();
        if !enemies.is_empty() {
            let t = enemies[rng.random_range(0..enemies.len())];
            let bullet_idx = rng.random_range(0..3);
            cmd.spawn((
                Sprite::from_atlas_image(
                    sprites.enemy_bullet_texture.clone(),
                    TextureAtlas {
                        layout: sprites.enemy_bullet_layout.clone(),
                        index: bullet_idx,
                    },
                ),
                Transform::from_xyz(t.translation.x, t.translation.y - ENEMY_SIZE.y / 2.0, 1.0)
                    .with_scale(Vec3::splat(SPRITE_SCALE * 0.5)),
                EnemyBullet,
                GameEntity,
            ));
        }
        gd.enemy_shoot_timer = Timer::from_seconds(gd.enemy_shoot_rate(), TimerMode::Once);
    }
}

pub fn initiate_dives(
    mut cmd: Commands,
    time: Res<Time>,
    mut gd: ResMut<GameData>,
    eq: FormationEnemyQuery,
    dq: Query<&DivingEnemy>,
    pq: Query<&Transform, With<Player>>,
    mut rng: Local<Option<SmallRng>>,
) {
    if gd.wave < 2 {
        return;
    }
    let rng = rng.get_or_insert_with(SmallRng::from_os_rng);
    gd.dive_timer.tick(time.delta());
    if gd.dive_timer.is_finished() {
        gd.dive_timer = Timer::from_seconds(gd.dive_interval(), TimerMode::Once);
        let (current, max) = (dq.iter().count(), gd.max_divers());
        if current >= max {
            return;
        }
        let Ok(pt) = pq.single() else { return };
        let candidates: Vec<_> = eq.iter().collect();
        if candidates.is_empty() {
            return;
        }
        for _ in 0..(max - current)
            .min(1 + gd.wave as usize / 3)
            .min(candidates.len())
        {
            let (e, t) = candidates[rng.random_range(0..candidates.len())];
            cmd.entity(e).insert(DivingEnemy {
                target_x: pt.translation.x + rng.random_range(-50.0..50.0),
                returning: false,
                start_y: t.translation.y,
                original_x: t.translation.x,
                progress: 0.0,
                amplitude: rng.random_range(60.0..120.0),
                curve_dir: if rng.random_bool(0.5) { 1.0 } else { -1.0 },
            });
        }
    }
}

pub fn diving_movement(
    mut cmd: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut Transform, &mut DivingEnemy)>,
    screen: Res<ScreenSize>,
) {
    let bottom = -screen.half_height + 40.0;
    let dt = time.delta_secs();

    for (e, mut t, mut d) in q.iter_mut() {
        if !d.returning {
            d.progress += dt * 0.8;
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
            d.progress += dt * 0.6;
            let p = d.progress.min(1.0);
            let ease = 1.0 - (1.0 - p).powi(2);
            let arc = (p * std::f32::consts::PI).sin();
            let target_x = d.target_x + (d.original_x - d.target_x) * ease;
            t.translation.x = target_x + arc * d.amplitude * 0.5 * -d.curve_dir;
            t.translation.y = bottom + (d.start_y - bottom) * ease;
            if d.progress >= 1.0 {
                t.translation = Vec3::new(d.original_x, d.start_y, 1.0);
                cmd.entity(e).remove::<DivingEnemy>();
            }
        }
    }
}

// === Collisions ===

pub fn collisions(
    mut cmd: Commands,
    queries: CollisionQueries,
    ctx: ShootingContext,
    fonts: Res<FontAssets>,
    mut events: CollisionEvents,
    mut shake: ResMut<ScreenShake>,
    mut combo: ResMut<ComboTracker>,
    invincible_player: Query<(), (With<Player>, With<Invincible>)>,
    mut rng: Local<Option<SmallRng>>,
) {
    let rng = rng.get_or_insert_with(SmallRng::from_os_rng);
    let player_is_invincible = !invincible_player.is_empty();

    for (be, bt) in queries.player_bullets.iter() {
        let bp = bt.translation.truncate();
        for (ee, et, enemy) in queries.enemies.iter() {
            if bp.distance(et.translation.truncate()) < BULLET_SIZE.y / 2.0 + ENEMY_SIZE.x / 2.0 {
                cmd.entity(be).try_despawn();
                cmd.entity(ee).try_despawn();
                spawn_explosion(&mut cmd, et.translation, &ctx.sprites);
                events.enemy_killed.write(EnemyKilledMsg(enemy.points));

                // Track combo and spawn popup
                combo.add_kill();
                shake.add_trauma(SHAKE_TRAUMA_EXPLOSION);
                spawn_score_popup(
                    &mut cmd,
                    &fonts,
                    et.translation,
                    enemy.points,
                    combo.current(),
                );

                // Random chance to spawn power-up (only if player doesn't have all)
                if rng.random::<f32>() < POWERUP_DROP_CHANCE {
                    spawn_powerup(&mut cmd, et.translation, &ctx.sprites, &ctx.powerups, rng);
                }
                break;
            }
        }
    }

    // Skip player-hit collisions if invincible
    if player_is_invincible {
        return;
    }

    let Ok(pt) = queries.player.single() else {
        return;
    };
    let pp = pt.translation.truncate();

    for (be, bt) in queries.enemy_bullets.iter() {
        if bt.translation.truncate().distance(pp) < BULLET_SIZE.y / 2.0 + PLAYER_SIZE.x / 2.0 {
            cmd.entity(be).try_despawn();
            spawn_explosion(&mut cmd, pt.translation, &ctx.sprites); // Explosion at player
            shake.add_trauma(SHAKE_TRAUMA_PLAYER_HIT);
            events.player_hit.write(PlayerHitMsg);
            return;
        }
    }

    for (de, dt) in queries.divers.iter() {
        if dt.translation.truncate().distance(pp) < ENEMY_SIZE.x / 2.0 + PLAYER_SIZE.x / 2.0 {
            cmd.entity(de).try_despawn();
            spawn_explosion(&mut cmd, pt.translation, &ctx.sprites); // Explosion at player
            shake.add_trauma(SHAKE_TRAUMA_PLAYER_HIT);
            events.player_hit.write(PlayerHitMsg);
            return;
        }
    }
}

/// Update combo timer
pub fn combo_tick(time: Res<Time>, mut combo: ResMut<ComboTracker>) {
    combo.tick(time.delta_secs());
}

pub fn check_wave_complete(
    eq: Query<Entity, With<Enemy>>,
    aq: Query<&FadingText>,
    mut events: MessageWriter<WaveCompleteMsg>,
) {
    if eq.is_empty() && aq.is_empty() {
        events.write(WaveCompleteMsg);
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

// === Global ===

pub fn game_restart(
    mut cmd: Commands,
    kb: Res<ButtonInput<KeyCode>>,
    mut state: GameStateMut,
    game_state: Res<State<GameState>>,
    mut fade: ResMut<MusicFade>,
    music_enabled: Res<MusicEnabled>,
    spawn: SpawnContext,
    cleanup: RestartCleanup,
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
        spawn_player(&mut cmd, spawn.screen.half_height, &spawn.sprites, false);
        spawn_fading_text(
            &mut cmd,
            &spawn.fonts,
            "WAVE 1",
            2.0,
            Color::srgba(1.0, 1.0, 0.2, 1.0),
            true,
        );

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

        if music_enabled.0 {
            request_subsong(&mut fade, 2);
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
) {
    if quit_state.showing {
        // Handle Y/N while confirmation is showing
        if kb.just_pressed(KeyCode::KeyY) {
            std::process::exit(0);
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

pub fn enter_name_entry(
    mut cmd: Commands,
    gd: Res<GameData>,
    spawn: SpawnContext,
    mut name_state: ResMut<NameEntryState>,
    mut powerups: ResMut<PowerUpState>,
    eq: Query<(Entity, &Transform), With<Enemy>>,
    cleanup: GameEndCleanup,
) {
    let mut rng = SmallRng::from_os_rng();

    cleanup_game_entities(&mut cmd, &mut powerups, cleanup.all(), std::iter::empty());

    let enemies: Vec<_> = eq.iter().collect();
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
            collected.write(PowerUpCollectedMsg(powerup.kind));
            sfx.write(PlaySfxMsg(SfxType::PowerUp));
        }
    }
}

pub fn handle_powerup_collected(
    mut cmd: Commands,
    mut events: MessageReader<PowerUpCollectedMsg>,
    mut powerups: ResMut<PowerUpState>,
    mut flash: ResMut<ScreenFlash>,
    sprites: Res<SpriteAssets>,
    screen: Res<ScreenSize>,
    player: Query<Entity, With<Player>>,
    side_boosters: Query<Entity, With<SideBooster>>,
) {
    for ev in events.read() {
        // Get flash color based on power-up type
        let flash_color = match ev.0 {
            PowerUpType::RapidFire => Color::srgba(1.0, 0.4, 0.4, 0.4),
            PowerUpType::TripleShot => Color::srgba(0.4, 1.0, 0.4, 0.4),
            PowerUpType::SpeedBoost => Color::srgba(0.4, 0.6, 1.0, 0.4),
            PowerUpType::PowerShot => Color::srgba(1.0, 1.0, 0.4, 0.4),
        };

        match ev.0 {
            PowerUpType::RapidFire => powerups.rapid_fire = true,
            PowerUpType::TripleShot => powerups.triple_shot = true,
            PowerUpType::SpeedBoost => powerups.speed_boost = true,
            PowerUpType::PowerShot => powerups.power_shot = true,
        }

        // Trigger screen flash
        flash.timer = 0.3;
        flash.color = flash_color;

        // Spawn flash overlay
        cmd.spawn((
            Sprite {
                color: flash_color,
                custom_size: Some(Vec2::new(screen.width * 2.0, screen.height * 2.0)),
                ..default()
            },
            Transform::from_xyz(0.0, 0.0, 50.0),
            ScreenFlashOverlay,
        ));

        // Spawn side boosters if we have any power-up and don't have them yet
        if powerups.has_any()
            && side_boosters.is_empty()
            && let Ok(player_id) = player.single()
        {
            spawn_side_boosters(&mut cmd, player_id, &sprites);
        }
    }
}

/// Update screen flash overlay
pub fn screen_flash_system(
    mut cmd: Commands,
    time: Res<Time>,
    mut flash: ResMut<ScreenFlash>,
    mut overlays: Query<(Entity, &mut Sprite), With<ScreenFlashOverlay>>,
) {
    if flash.timer > 0.0 {
        flash.timer -= time.delta_secs();

        // Fade out the overlay
        let alpha = (flash.timer / 0.3).max(0.0) * 0.4;
        for (entity, mut sprite) in overlays.iter_mut() {
            let base = flash.color.to_srgba();
            sprite.color = Color::srgba(base.red, base.green, base.blue, alpha);

            if flash.timer <= 0.0 {
                cmd.entity(entity).try_despawn();
            }
        }
    }
}

/// Handle invincibility timer and flicker effect
pub fn invincibility_system(
    mut cmd: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut Invincible, &mut Sprite), With<Player>>,
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
        }
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
        format!("{} x{}", points, combo)
    } else {
        format!("{}", points)
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
    mut q: Query<(&mut Transform, &Star, &StarLayer)>,
) {
    let dt = time.delta_secs();
    let hh = screen.half_height;

    for (mut transform, star, layer) in q.iter_mut() {
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
    }
}

/// Slow-moving nebula clouds for deep parallax effect
pub fn nebula_movement(
    time: Res<Time>,
    screen: Res<ScreenSize>,
    mut q: Query<(&mut Transform, &Nebula)>,
) {
    let dt = time.delta_secs();
    let hh = screen.half_height;

    for (mut transform, nebula) in q.iter_mut() {
        transform.translation.y -= nebula.speed * dt;

        // Wrap around (with larger margin for big sprites)
        if transform.translation.y < -hh - 200.0 {
            transform.translation.y = hh + 200.0;
        }
    }
}
