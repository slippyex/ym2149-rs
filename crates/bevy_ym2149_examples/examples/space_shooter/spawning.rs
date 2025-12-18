//! Entity spawning helpers

use bevy::prelude::*;

use rand::Rng;
use rand::rngs::SmallRng;

use super::components::*;
use super::config::*;
use super::resources::*;

pub const INVINCIBILITY_DURATION: f32 = 10.0;

pub fn spawn_player(
    cmd: &mut Commands,
    hh: f32,
    sprites: &SpriteAssets,
    invincible: bool,
) -> Entity {
    let player_y = -hh + 60.0;
    let mut player_cmd = cmd.spawn((
        Sprite::from_atlas_image(
            sprites.player_texture.clone(),
            TextureAtlas {
                layout: sprites.player_layout.clone(),
                index: 1,
            },
        ),
        Transform::from_xyz(0.0, player_y, 1.0).with_scale(Vec3::splat(SPRITE_SCALE)),
        Player,
        GameEntity,
    ));

    if invincible {
        player_cmd.insert(Invincible::new(INVINCIBILITY_DURATION));
    }

    let player_id = player_cmd.id();

    // Booster flame as child
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
    ));

    // Shield bubble when invincible
    if invincible {
        cmd.spawn((
            Sprite {
                image: sprites.bubble_texture.clone(),
                color: Color::srgba(1.0, 0.9, 0.2, 0.5), // Yellow tint
                ..default()
            },
            Transform::from_xyz(0.0, player_y, 0.9), // Slightly behind player
            ShieldBubble,
            GameEntity,
        ));
    }

    player_id
}

pub fn spawn_enemies_for_wave(
    cmd: &mut Commands,
    sprites: &SpriteAssets,
    screen: &ScreenSize,
    wave: u32,
    pattern_offset: usize,
) {
    if wave % BOSS_WAVE_INTERVAL == 0 {
        spawn_boss_for_wave(cmd, sprites, screen, wave);
        return;
    }

    // 8 columns; bitmask per row (LSB = col 0).
    const PATTERNS: [[u8; 4]; 6] = [
        // Wave 1: classic full block.
        [0b1111_1111, 0b1111_1111, 0b1111_1111, 0b1111_1111],
        // Split lanes (forces movement).
        [0b1110_0111, 0b1110_0111, 0b1100_0011, 0b1110_0111],
        // Checkerboard (aiming, less "hold fire").
        [0b1010_1010, 0b0101_0101, 0b1010_1010, 0b0101_0101],
        // Diamond-ish.
        [0b0011_1100, 0b0111_1110, 0b1111_1111, 0b0111_1110],
        // Wings (two clusters).
        [0b1110_0111, 0b1100_0011, 0b1110_0111, 0b1100_0011],
        // Columns (big dodge windows, but more shooting pressure later).
        [0b1010_1010, 0b1010_1010, 0b1010_1010, 0b1010_1010],
    ];

    // Wave 1 is always the classic full block. After that we "rotate" patterns per run
    // (offset) while gradually unlocking more complex patterns as waves increase.
    let pattern = if wave <= 1 {
        PATTERNS[0]
    } else {
        let unlocked = (1 + ((wave.saturating_sub(2)) as usize / 2)).min(PATTERNS.len() - 1);
        let idx = 1 + (pattern_offset + (wave as usize - 2)) % unlocked.max(1);
        PATTERNS[idx]
    };
    let extra_rows = (wave / 4).min(2) as usize; // +0..2 rows as difficulty ramps
    let total_rows = 4 + extra_rows;

    let start_x = -(7.0 * ENEMY_SPACING.x) / 2.0;
    // Keep enemies safely above the player zone, regardless of window height.
    let min_bottom_y = -screen.half_height + 220.0;
    let base_y_target =
        (screen.half_height - 170.0) - (wave.saturating_sub(1) as f32 * 6.0).min(34.0);
    let base_y =
        base_y_target.max(min_bottom_y + (total_rows.saturating_sub(1) as f32) * ENEMY_SPACING.y);

    for (row, &row_mask) in pattern.iter().enumerate().take(total_rows.min(4)) {
        let (pts, enemy_type) = match row {
            0 => (100, EnemyType::Lips),
            1 => (50, EnemyType::BonBon),
            _ => (25, EnemyType::Alan),
        };
        let hp = enemy_hp_for_row(wave, row);
        let (texture, layout, last_frame) = match enemy_type {
            EnemyType::Alan => (
                sprites.enemy_alan_texture.clone(),
                sprites.enemy_alan_layout.clone(),
                5,
            ),
            EnemyType::BonBon => (
                sprites.enemy_bonbon_texture.clone(),
                sprites.enemy_bonbon_layout.clone(),
                3,
            ),
            EnemyType::Lips => (
                sprites.enemy_lips_texture.clone(),
                sprites.enemy_lips_layout.clone(),
                4,
            ),
        };

        // Stagger some patterns slightly per row.
        let stagger = if wave % 2 == 0 && (row % 2 == 1) {
            ENEMY_SPACING.x * 0.5
        } else {
            0.0
        };

        let mask = row_mask;
        for col in 0..8 {
            if (mask >> col) & 1 == 0 {
                continue;
            }

            cmd.spawn((
                Sprite::from_atlas_image(
                    texture.clone(),
                    TextureAtlas {
                        layout: layout.clone(),
                        index: 0,
                    },
                ),
                Transform::from_xyz(
                    start_x + col as f32 * ENEMY_SPACING.x + stagger,
                    base_y - row as f32 * ENEMY_SPACING.y,
                    1.0,
                )
                .with_scale(Vec3::splat(SPRITE_SCALE)),
                Enemy { points: pts },
                EnemyHp { current: hp },
                GameEntity,
                AnimationIndices {
                    first: 0,
                    last: last_frame,
                },
                AnimationTimer(Timer::from_seconds(1.0 / ANIM_FPS, TimerMode::Repeating)),
            ));
        }
    }

    // Extra rows beyond the 4-row pattern are always "full".
    for row in 4..total_rows {
        let pts = 25;
        let hp = enemy_hp_for_row(wave, row);
        let (texture, layout, last_frame) = (
            sprites.enemy_alan_texture.clone(),
            sprites.enemy_alan_layout.clone(),
            5,
        );

        let stagger = if wave % 2 == 0 && (row % 2 == 1) {
            ENEMY_SPACING.x * 0.5
        } else {
            0.0
        };

        let mask = 0b1111_1111;
        for col in 0..8 {
            if (mask >> col) & 1 == 0 {
                continue;
            }

            cmd.spawn((
                Sprite::from_atlas_image(
                    texture.clone(),
                    TextureAtlas {
                        layout: layout.clone(),
                        index: 0,
                    },
                ),
                Transform::from_xyz(
                    start_x + col as f32 * ENEMY_SPACING.x + stagger,
                    base_y - row as f32 * ENEMY_SPACING.y,
                    1.0,
                )
                .with_scale(Vec3::splat(SPRITE_SCALE)),
                Enemy { points: pts },
                EnemyHp { current: hp },
                GameEntity,
                AnimationIndices {
                    first: 0,
                    last: last_frame,
                },
                AnimationTimer(Timer::from_seconds(1.0 / ANIM_FPS, TimerMode::Repeating)),
            ));
        }
    }
}

fn enemy_hp_for_row(wave: u32, row: usize) -> u8 {
    // Keep early waves snappy; gradually add durability so waves aren't "hold fire to win".
    // Baseline ramps for the whole formation after wave 4.
    let base = 1 + (wave.saturating_sub(4) / 4).min(3) as u8; // 1..4
    let row_bonus = match row {
        // Top rows ramp earlier and harder.
        0 => (wave / 5).min(2) as u8,  // +0..2
        1 => (wave / 8).min(2) as u8,  // +0..2
        2 => (wave / 12).min(1) as u8, // +0..1
        _ => 0,
    };
    (base + row_bonus).clamp(1, 6)
}

fn boss_frame_index(stage: u32) -> usize {
    // 2x3 atlas (indices 0..5). Cycle through variants as stages climb.
    ((stage.saturating_sub(1)) as usize) % 6
}

fn boss_max_hp(stage: u32) -> i32 {
    // Not too spongy; ramps steadily.
    (44 + (stage.saturating_sub(1) as i32) * 10).clamp(44, 96)
}

fn boss_points(stage: u32) -> u32 {
    2000 + (stage.saturating_sub(1) * 600)
}

fn boss_shoot_interval(stage: u32) -> f32 {
    (1.35 - (stage.saturating_sub(1) as f32) * 0.08).max(0.75)
}

pub fn spawn_boss_for_wave(
    cmd: &mut Commands,
    sprites: &SpriteAssets,
    screen: &ScreenSize,
    wave: u32,
) {
    let stage = (wave / BOSS_WAVE_INTERVAL).max(1);
    let hp = boss_max_hp(stage);
    let y = screen.half_height - 160.0;

    cmd.spawn((
        Sprite::from_atlas_image(
            sprites.boss_texture.clone(),
            TextureAtlas {
                layout: sprites.boss_layout.clone(),
                index: boss_frame_index(stage),
            },
        ),
        Transform::from_xyz(0.0, y, 1.0).with_scale(Vec3::splat(BOSS_SCALE)),
        Boss {
            stage,
            points: boss_points(stage),
        },
        BossEnergy {
            current: hp,
            max: hp,
        },
        BossWeapon {
            timer: Timer::from_seconds(boss_shoot_interval(stage), TimerMode::Once),
        },
        BossTarget {
            target: Vec2::new(0.0, y),
            timer: Timer::from_seconds(0.15, TimerMode::Once),
        },
        GameEntity,
    ));

    // Escorted bosses on later stages.
    let escorts = match stage {
        0 | 1 => 0,
        2 => 2,
        _ => 4,
    };
    if escorts > 0 {
        let escort_radius = 86.0;
        let escort_speed = 1.2 + (stage as f32) * 0.08;
        for i in 0..escorts {
            let angle = (i as f32 / escorts as f32) * std::f32::consts::TAU;
            let escort_hp = (2 + (stage / 3).min(2) as u8).min(4);
            cmd.spawn((
                Sprite::from_atlas_image(
                    sprites.enemy_bonbon_texture.clone(),
                    TextureAtlas {
                        layout: sprites.enemy_bonbon_layout.clone(),
                        index: 0,
                    },
                ),
                Transform::from_xyz(0.0, y, 1.1).with_scale(Vec3::splat(SPRITE_SCALE)),
                Enemy { points: 150 },
                EnemyHp { current: escort_hp },
                BossEscort {
                    angle,
                    radius: escort_radius,
                    speed: escort_speed * (if i % 2 == 0 { 1.0 } else { -1.0 }),
                },
                EscortShooter {
                    timer: Timer::from_seconds(0.8 + (i as f32) * 0.15, TimerMode::Once),
                },
                AnimationIndices { first: 0, last: 3 },
                AnimationTimer(Timer::from_seconds(1.0 / ANIM_FPS, TimerMode::Repeating)),
                GameEntity,
            ));
        }
    }

    spawn_boss_bar(cmd, screen);
}

pub fn spawn_boss_bar(cmd: &mut Commands, screen: &ScreenSize) {
    let width = 260.0;
    let height = 12.0;
    let left_x = -width * 0.5;
    let y = screen.half_height - 30.0;

    cmd.spawn((
        Sprite {
            color: Color::srgba(0.0, 0.0, 0.0, 0.6),
            custom_size: Some(Vec2::new(width + 6.0, height + 6.0)),
            ..default()
        },
        Transform::from_xyz(0.0, y, 10.0),
        BossBarBg,
        GameEntity,
    ));

    let fill_width = width;
    cmd.spawn((
        Sprite {
            color: Color::srgb(1.0, 0.35, 0.35),
            custom_size: Some(Vec2::new(fill_width, height)),
            ..default()
        },
        Transform::from_xyz(left_x + fill_width * 0.5, y, 11.0),
        BossBarFill {
            left_x,
            width,
            height,
        },
        GameEntity,
    ));
}

pub fn spawn_fading_text(
    cmd: &mut Commands,
    fonts: &FontAssets,
    text: &str,
    duration: f32,
    color: Color,
    spawn_enemies: bool,
) {
    if spawn_enemies {
        cmd.spawn((
            Sprite {
                color: Color::NONE,
                custom_size: Some(Vec2::ZERO),
                ..default()
            },
            Transform::default(),
            FadingText {
                timer: Timer::from_seconds(duration, TimerMode::Once),
                color,
                spawn_enemies: true,
            },
            GameEntity,
        ));
    }

    if text.is_empty() {
        return;
    }

    cmd.spawn((
        Text::new(text),
        TextFont {
            font: fonts.arcade.clone(),
            font_size: if spawn_enemies { 72.0 } else { 48.0 },
            ..default()
        },
        TextColor(color),
        TextLayout::new_with_justify(bevy::text::Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(if spawn_enemies { 45.0 } else { 40.0 }),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        FadingText {
            timer: Timer::from_seconds(duration, TimerMode::Once),
            color,
            spawn_enemies: false,
        },
    ));
}

pub fn spawn_explosion(cmd: &mut Commands, pos: Vec3, sprites: &SpriteAssets) {
    // Ring burst for extra impact (cheap + very readable through CRT).
    cmd.spawn((
        Sprite {
            image: sprites.ring_texture.clone(),
            color: Color::srgba(1.0, 0.9, 0.35, 0.35),
            ..default()
        },
        Transform::from_translation(pos + Vec3::new(0.0, 0.0, 0.8))
            .with_scale(Vec3::splat(SPRITE_SCALE * 0.35)),
        ExplosionRing {
            timer: Timer::from_seconds(0.22, TimerMode::Once),
            start_scale: SPRITE_SCALE * 0.35,
            end_scale: SPRITE_SCALE * 1.4,
            start_alpha: 0.35,
        },
        GameEntity,
    ));

    cmd.spawn((
        Sprite::from_atlas_image(
            sprites.explosion_texture.clone(),
            TextureAtlas {
                layout: sprites.explosion_layout.clone(),
                index: 0,
            },
        ),
        Transform::from_translation(pos).with_scale(Vec3::splat(SPRITE_SCALE)),
        Explosion,
        GameEntity,
        AnimationIndices { first: 0, last: 4 },
        AnimationTimer(Timer::from_seconds(0.08, TimerMode::Repeating)),
    ));
}

/// Get the atlas index for a digit (0-9)
/// Number font layout: 1-5 in top row (indices 0-4), 6-9,0 in bottom row (indices 5-9)
pub fn digit_to_atlas_index(digit: u8) -> usize {
    if digit == 0 { 9 } else { (digit - 1) as usize }
}

pub fn spawn_life_icons(
    cmd: &mut Commands,
    sprites: &SpriteAssets,
    screen: &ScreenSize,
    lives: u32,
) {
    let base_x = screen.half_width - 30.0;
    let y = screen.half_height - 30.0;

    for i in 0..lives {
        let x = base_x - (i as f32) * LIFE_ICON_SPACING;
        cmd.spawn((
            Sprite::from_image(sprites.life_icon_texture.clone()),
            Transform::from_xyz(x, y, 10.0).with_scale(Vec3::splat(LIFE_ICON_SCALE)),
            LifeIcon,
            GameEntity,
        ));
    }
}

pub fn spawn_score_digits(
    cmd: &mut Commands,
    sprites: &SpriteAssets,
    screen: &ScreenSize,
    score_type: ScoreType,
    value: u32,
) {
    let (base_x, y) = match score_type {
        ScoreType::Score => (-screen.half_width + 30.0, screen.half_height - 25.0),
        ScoreType::HighScore => (-40.0, screen.half_height - 25.0),
    };
    for (i, c) in format!("{:06}", value.min(999999)).chars().enumerate() {
        let digit = c.to_digit(10).unwrap_or(0) as u8;
        cmd.spawn((
            Sprite::from_atlas_image(
                sprites.number_font_texture.clone(),
                TextureAtlas {
                    layout: sprites.number_font_layout.clone(),
                    index: digit_to_atlas_index(digit),
                },
            ),
            Transform::from_xyz(base_x + (i as f32) * DIGIT_SPACING, y, 10.0)
                .with_scale(Vec3::splat(DIGIT_SCALE)),
            DigitSprite { position: i },
            score_type,
            GameEntity,
        ));
    }
}

pub fn spawn_wave_digits(
    cmd: &mut Commands,
    sprites: &SpriteAssets,
    screen: &ScreenSize,
    wave: u32,
) {
    let (base_x, y) = (-screen.half_width + 30.0, screen.half_height - 60.0);
    for (i, c) in format!("{:02}", wave.min(99)).chars().enumerate() {
        let digit = c.to_digit(10).unwrap_or(0) as u8;
        cmd.spawn((
            Sprite::from_atlas_image(
                sprites.number_font_texture.clone(),
                TextureAtlas {
                    layout: sprites.number_font_layout.clone(),
                    index: digit_to_atlas_index(digit),
                },
            ),
            Transform::from_xyz(base_x + (i as f32) * 22.0, y, 10.0)
                .with_scale(Vec3::splat(WAVE_DIGIT_SCALE)),
            WaveDigit { position: i },
            GameEntity,
        ));
    }
}

pub fn spawn_energy_bar(cmd: &mut Commands, screen: &ScreenSize, energy: &PlayerEnergy) {
    // Top-left, below the wave digits.
    let left_x = -screen.half_width + 30.0;
    let y = screen.half_height - 90.0;
    let width = 120.0;
    let height = 10.0;

    cmd.spawn((
        Sprite {
            color: Color::srgba(0.0, 0.0, 0.0, 0.55),
            custom_size: Some(Vec2::new(width + 4.0, height + 4.0)),
            ..default()
        },
        Transform::from_xyz(left_x + width * 0.5, y, 10.0),
        EnergyBarBg,
        GameEntity,
    ));

    let fill_width = (energy.fraction() * width).clamp(0.0, width);
    cmd.spawn((
        Sprite {
            color: Color::srgb(0.35, 1.0, 0.45),
            custom_size: Some(Vec2::new(fill_width, height)),
            ..default()
        },
        Transform::from_xyz(left_x + fill_width * 0.5, y, 11.0),
        EnergyBarFill {
            left_x,
            width,
            height,
        },
        GameEntity,
    ));
}

/// Power-up visual configuration: (first_frame, last_frame) for animation
/// Bonuses-0001.png is 5x5 grid (32x32 sprites): columns are powerup types, rows are 5 animation frames
/// Row-major indexing: column 0 = [0,5,10,15,20], column 1 = [1,6,11,16,21], etc.
pub const POWERUP_ANIM: [(usize, usize); 5] = [
    (0, 20), // Heal - green cross (column 0)
    (2, 22), // RapidFire - red star (column 2)
    (4, 24), // TripleShot - magenta (column 4)
    (1, 21), // SpeedBoost - blue shield (column 1)
    (3, 23), // PowerShot - yellow 2x (column 3)
];

fn powerup_visual_index(kind: PowerUpType) -> usize {
    match kind {
        PowerUpType::Heal => 0,
        PowerUpType::RapidFire => 1,
        PowerUpType::TripleShot => 2,
        PowerUpType::SpeedBoost => 3,
        PowerUpType::PowerShot => 4,
    }
}

pub fn spawn_powerup_kind(
    cmd: &mut Commands,
    pos: Vec3,
    sprites: &SpriteAssets,
    kind: PowerUpType,
) {
    let visual_idx = powerup_visual_index(kind);
    let (first, last) = POWERUP_ANIM[visual_idx];

    cmd.spawn((
        Sprite {
            image: sprites.powerup_texture.clone(),
            texture_atlas: Some(TextureAtlas {
                layout: sprites.powerup_layout.clone(),
                index: first,
            }),
            ..default()
        },
        Transform::from_translation(pos).with_scale(Vec3::splat(POWERUP_SCALE)),
        PowerUp { kind },
        GameEntity,
        PowerUpAnimation { first, last },
        AnimationTimer(Timer::from_seconds(0.15, TimerMode::Repeating)),
    ));
}

/// Spawn a random power-up that the player doesn't have yet
pub fn spawn_powerup(
    cmd: &mut Commands,
    pos: Vec3,
    sprites: &SpriteAssets,
    powerups: &PowerUpState,
    energy: &PlayerEnergy,
    on_screen_kinds: &[PowerUpType],
    rng: &mut SmallRng,
) {
    // Candidates are kinds. Heal is consumable and only drops if not full.
    let mut candidates: [PowerUpType; 5] = [
        PowerUpType::Heal,
        PowerUpType::RapidFire,
        PowerUpType::TripleShot,
        PowerUpType::SpeedBoost,
        PowerUpType::PowerShot,
    ];
    let mut count = 0usize;
    // Heal is intentionally rare and mostly appears when the player is low.
    if energy.current > 0
        && energy.current <= 2
        && !energy.is_full()
        && !on_screen_kinds.contains(&PowerUpType::Heal)
        && rng.random::<f32>() < 0.30
    {
        candidates[count] = PowerUpType::Heal;
        count += 1;
    }
    if !powerups.rapid_fire {
        let kind = PowerUpType::RapidFire;
        if !on_screen_kinds.contains(&kind) {
            candidates[count] = kind;
            count += 1;
        }
    }
    if !powerups.triple_shot {
        let kind = PowerUpType::TripleShot;
        if !on_screen_kinds.contains(&kind) {
            candidates[count] = kind;
            count += 1;
        }
    }
    if !powerups.speed_boost {
        let kind = PowerUpType::SpeedBoost;
        if !on_screen_kinds.contains(&kind) {
            candidates[count] = kind;
            count += 1;
        }
    }
    if !powerups.power_shot {
        let kind = PowerUpType::PowerShot;
        if !on_screen_kinds.contains(&kind) {
            candidates[count] = kind;
            count += 1;
        }
    }

    if count == 0 {
        return;
    };
    let kind = candidates[rng.random_range(0..count)];

    spawn_powerup_kind(cmd, pos, sprites, kind);
}

/// Spawn side boosters attached to the player
pub fn spawn_side_boosters(cmd: &mut Commands, player_id: Entity, sprites: &SpriteAssets) {
    let boosters = [
        (
            -10.0,
            &sprites.booster_left_texture,
            &sprites.booster_left_layout,
        ),
        (
            10.0,
            &sprites.booster_right_texture,
            &sprites.booster_right_layout,
        ),
    ];
    for (x, texture, layout) in boosters {
        cmd.spawn((
            Sprite::from_atlas_image(
                texture.clone(),
                TextureAtlas {
                    layout: layout.clone(),
                    index: 0,
                },
            ),
            Transform::from_xyz(x, -8.0, -0.1),
            ChildOf(player_id),
            SideBooster,
            AnimationIndices { first: 0, last: 1 },
            AnimationTimer(Timer::from_seconds(0.08, TimerMode::Repeating)),
        ));
    }
}
