//! Gameplay systems

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_ym2149::Ym2149Playback;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

use super::components::*;
use super::config::*;
use super::resources::*;
use super::spawning::*;

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
    if kb.just_pressed(KeyCode::Escape) {
        std::process::exit(0);
    }
}

pub fn title_anim(time: Res<Time>, mut q: Query<(&mut TextColor, &UiMarker)>) {
    for (mut c, m) in q.iter_mut() {
        if *m == UiMarker::PressEnter {
            c.0 = Color::srgba(1.0, 1.0, 0.2, (time.elapsed_secs() * 2.0).sin() * 0.3 + 0.7);
        }
    }
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

pub fn enter_playing(
    mut cmd: Commands,
    mut gd: ResMut<GameData>,
    mut fade: ResMut<MusicFade>,
    screen: Res<ScreenSize>,
    sprites: Res<SpriteAssets>,
    mut uiq: Query<(&mut Visibility, &UiMarker)>,
) {
    request_subsong(&mut fade, 2);
    gd.reset();
    spawn_player(&mut cmd, screen.half_height, &sprites);
    spawn_fading_text(&mut cmd, "WAVE 1", 2.0, Color::srgba(1.0, 1.0, 0.2, 1.0), true);

    spawn_life_icons(&mut cmd, &sprites, &screen, gd.lives);
    spawn_score_digits(&mut cmd, &sprites, &screen, ScoreType::Score, gd.score);
    spawn_score_digits(&mut cmd, &sprites, &screen, ScoreType::HighScore, gd.high_score);
    spawn_wave_digits(&mut cmd, &sprites, &screen, gd.wave);

    for (mut v, _) in uiq.iter_mut() {
        *v = Visibility::Hidden;
    }
}

pub fn enter_gameover(
    mut cmd: Commands,
    mut fade: ResMut<MusicFade>,
    mut uiq: Query<(Entity, &mut Visibility, &UiMarker)>,
    eq: Query<(Entity, &Transform), With<Enemy>>,
    player_bullets: Query<Entity, With<PlayerBullet>>,
    enemy_bullets: Query<Entity, With<EnemyBullet>>,
    mut rng: Local<Option<SmallRng>>,
) {
    let rng = rng.get_or_insert_with(SmallRng::from_os_rng);
    request_subsong(&mut fade, 3);

    for e in player_bullets.iter().chain(enemy_bullets.iter()) {
        cmd.entity(e).despawn();
    }

    for (entity, mut v, m) in uiq.iter_mut() {
        if *m == UiMarker::GameOver {
            *v = Visibility::Visible;
            cmd.entity(entity).insert(GameOverUi { base_top: 45.0 });
        }
    }

    let mut enemies: Vec<_> = eq.iter().collect();
    for i in (1..enemies.len()).rev() {
        let j = rng.random_range(0..=i);
        enemies.swap(i, j);
    }
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

pub fn exit_gameover(mut cmd: Commands, mut q: Query<(Entity, &mut Visibility, &UiMarker)>) {
    for (entity, mut v, m) in q.iter_mut() {
        if *m == UiMarker::GameOver {
            *v = Visibility::Hidden;
            cmd.entity(entity).remove::<GameOverUi>();
        }
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
) {
    for _ in events.read() {
        gd.wave += 1;
        gd.enemy_direction = 1.0;
        gd.dive_timer = Timer::from_seconds(gd.dive_interval(), TimerMode::Once);
        spawn_fading_text(
            &mut cmd,
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
    mut gd: ResMut<GameData>,
    mut ns: ResMut<NextState<GameState>>,
    pq: Query<Entity, With<Player>>,
    screen: Res<ScreenSize>,
    sprites: Res<SpriteAssets>,
    mut sfx: MessageWriter<PlaySfxMsg>,
) {
    for _ in events.read() {
        sfx.write(PlaySfxMsg(SfxType::Death));
        gd.lives = gd.lives.saturating_sub(1);
        for e in pq.iter() {
            cmd.entity(e).despawn();
        }
        if gd.lives == 0 {
            ns.set(GameState::GameOver);
        } else {
            spawn_player(&mut cmd, screen.half_height, &sprites);
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
) {
    for _ in events.read() {
        gd.lives += 1;
        spawn_fading_text(&mut cmd, "LIVES +1", 1.0, Color::srgba(0.2, 1.0, 0.2, 1.0), false);
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
            cmd.entity(entity).despawn();
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
) {
    let Ok((mut t, mut sprite)) = q.single_mut() else {
        return;
    };
    let hw = screen.half_width - PLAYER_SIZE.x / 2.0;
    let dir = kb.pressed(KeyCode::ArrowRight) as i32 - kb.pressed(KeyCode::ArrowLeft) as i32;
    t.translation.x =
        (t.translation.x + dir as f32 * PLAYER_SPEED * time.delta_secs()).clamp(-hw, hw);

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
    sprites: Res<SpriteAssets>,
    mut sfx: MessageWriter<PlaySfxMsg>,
) {
    gd.shoot_timer.tick(time.delta());
    if kb.pressed(KeyCode::Space)
        && gd.shoot_timer.is_finished()
        && let Ok(pt) = pq.single()
    {
        cmd.spawn((
            Sprite::from_atlas_image(
                sprites.player_bullet_texture.clone(),
                TextureAtlas {
                    layout: sprites.player_bullet_layout.clone(),
                    index: 0,
                },
            ),
            Transform::from_xyz(pt.translation.x, pt.translation.y + PLAYER_SIZE.y / 2.0, 1.0)
                .with_scale(Vec3::splat(SPRITE_SCALE * 0.5)),
            PlayerBullet,
            GameEntity,
            AnimationIndices { first: 0, last: 1 },
            AnimationTimer(Timer::from_seconds(0.05, TimerMode::Repeating)),
        ));
        sfx.write(PlaySfxMsg(SfxType::Laser));
        gd.shoot_timer = Timer::from_seconds(0.25, TimerMode::Once);
    }
}

pub fn bullet_movement(
    mut cmd: Commands,
    time: Res<Time>,
    screen: Res<ScreenSize>,
    mut player_bullets: Query<(Entity, &mut Transform), With<PlayerBullet>>,
    mut enemy_bullets: Query<(Entity, &mut Transform), (With<EnemyBullet>, Without<PlayerBullet>)>,
) {
    let hh = screen.half_height;
    for (e, mut t) in player_bullets.iter_mut() {
        t.translation.y += BULLET_SPEED * time.delta_secs();
        if t.translation.y > hh + 20.0 {
            cmd.entity(e).despawn();
        }
    }
    for (e, mut t) in enemy_bullets.iter_mut() {
        t.translation.y -= ENEMY_BULLET_SPEED * time.delta_secs();
        if t.translation.y < -hh - 20.0 {
            cmd.entity(e).despawn();
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
    eq: Query<(Entity, &Transform), (With<Enemy>, Without<DivingEnemy>)>,
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
    player_bullets: Query<(Entity, &Transform), With<PlayerBullet>>,
    enemy_bullets: Query<(Entity, &Transform), With<EnemyBullet>>,
    eq: Query<(Entity, &Transform, &Enemy)>,
    pq: Query<&Transform, With<Player>>,
    dq: Query<(Entity, &Transform), With<DivingEnemy>>,
    sprites: Res<SpriteAssets>,
    mut enemy_killed: MessageWriter<EnemyKilledMsg>,
    mut player_hit: MessageWriter<PlayerHitMsg>,
) {
    for (be, bt) in player_bullets.iter() {
        let bp = bt.translation.truncate();
        for (ee, et, enemy) in eq.iter() {
            if bp.distance(et.translation.truncate()) < BULLET_SIZE.y / 2.0 + ENEMY_SIZE.x / 2.0 {
                cmd.entity(be).despawn();
                cmd.entity(ee).despawn();
                spawn_explosion(&mut cmd, et.translation, &sprites);
                enemy_killed.write(EnemyKilledMsg(enemy.points));
                break;
            }
        }
    }

    let Ok(pt) = pq.single() else { return };
    let pp = pt.translation.truncate();

    for (be, bt) in enemy_bullets.iter() {
        if bt.translation.truncate().distance(pp) < BULLET_SIZE.y / 2.0 + PLAYER_SIZE.x / 2.0 {
            cmd.entity(be).despawn();
            player_hit.write(PlayerHitMsg);
            return;
        }
    }

    for (de, dt) in dq.iter() {
        if dt.translation.truncate().distance(pp) < ENEMY_SIZE.x / 2.0 + PLAYER_SIZE.x / 2.0 {
            cmd.entity(de).despawn();
            spawn_explosion(&mut cmd, dt.translation, &sprites);
            player_hit.write(PlayerHitMsg);
            return;
        }
    }
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
        if timer.just_finished() {
            if let Some(atlas) = &mut sprite.texture_atlas {
                atlas.index = if atlas.index >= indices.last {
                    indices.first
                } else {
                    atlas.index + 1
                };
            }
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
        if timer.just_finished() {
            if let Some(atlas) = &sprite.texture_atlas {
                if atlas.index >= indices.last {
                    cmd.entity(entity).despawn();
                }
            }
        }
    }
}

// === Global ===

pub fn starfield(time: Res<Time>, mut q: Query<(&mut Transform, &Star)>, screen: Res<ScreenSize>) {
    let hh = screen.half_height;
    for (mut t, s) in q.iter_mut() {
        t.translation.y -= s.speed * time.delta_secs();
        if t.translation.y < -hh {
            t.translation.y = hh;
        }
    }
}

pub fn game_input(
    mut cmd: Commands,
    kb: Res<ButtonInput<KeyCode>>,
    mut gd: ResMut<GameData>,
    mut ns: ResMut<NextState<GameState>>,
    state: Res<State<GameState>>,
    eq: Query<Entity, With<Enemy>>,
    player_bullets: Query<Entity, With<PlayerBullet>>,
    enemy_bullets: Query<Entity, With<EnemyBullet>>,
    pq: Query<Entity, With<Player>>,
    aq: Query<Entity, With<FadingText>>,
    life_icons: Query<Entity, With<LifeIcon>>,
    digit_sprites: Query<Entity, With<DigitSprite>>,
    wave_digits: Query<Entity, With<WaveDigit>>,
    screen: Res<ScreenSize>,
    sprites: Res<SpriteAssets>,
    mut fade: ResMut<MusicFade>,
) {
    if kb.just_pressed(KeyCode::KeyR) {
        for e in eq
            .iter()
            .chain(player_bullets.iter())
            .chain(enemy_bullets.iter())
            .chain(aq.iter())
            .chain(life_icons.iter())
            .chain(digit_sprites.iter())
            .chain(wave_digits.iter())
        {
            cmd.entity(e).despawn();
        }
        for e in pq.iter() {
            cmd.entity(e).despawn();
        }
        gd.reset();
        spawn_player(&mut cmd, screen.half_height, &sprites);
        spawn_fading_text(&mut cmd, "WAVE 1", 2.0, Color::srgba(1.0, 1.0, 0.2, 1.0), true);

        spawn_life_icons(&mut cmd, &sprites, &screen, gd.lives);
        spawn_score_digits(&mut cmd, &sprites, &screen, ScoreType::Score, gd.score);
        spawn_score_digits(&mut cmd, &sprites, &screen, ScoreType::HighScore, gd.high_score);
        spawn_wave_digits(&mut cmd, &sprites, &screen, gd.wave);

        request_subsong(&mut fade, 2);
        if *state.get() == GameState::GameOver {
            ns.set(GameState::Playing);
        }
    }
    if kb.just_pressed(KeyCode::Escape) {
        std::process::exit(0);
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

pub fn music_crossfade(time: Res<Time>, mut fade: ResMut<MusicFade>, mut q: Query<&mut Ym2149Playback>) {
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
