//! Entity spawning helpers

use bevy::prelude::*;

use rand::rngs::SmallRng;
use rand::Rng;

use super::components::*;
use super::config::*;
use super::resources::*;

pub fn spawn_player(cmd: &mut Commands, hh: f32, sprites: &SpriteAssets) {
    let player_id = cmd
        .spawn((
            Sprite::from_atlas_image(
                sprites.player_texture.clone(),
                TextureAtlas {
                    layout: sprites.player_layout.clone(),
                    index: 1,
                },
            ),
            Transform::from_xyz(0.0, -hh + 60.0, 1.0).with_scale(Vec3::splat(SPRITE_SCALE)),
            Player,
            GameEntity,
        ))
        .id();

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
}

pub fn spawn_enemies(cmd: &mut Commands, sprites: &SpriteAssets) {
    let start_x = -(7.0 * ENEMY_SPACING.x) / 2.0;
    let rows: [(u32, EnemyType); 4] = [
        (40, EnemyType::Lips),
        (30, EnemyType::BonBon),
        (20, EnemyType::Alan),
        (10, EnemyType::Alan),
    ];

    for (row, (pts, enemy_type)) in rows.iter().enumerate() {
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

        for col in 0..8 {
            cmd.spawn((
                Sprite::from_atlas_image(
                    texture.clone(),
                    TextureAtlas {
                        layout: layout.clone(),
                        index: 0,
                    },
                ),
                Transform::from_xyz(
                    start_x + col as f32 * ENEMY_SPACING.x,
                    200.0 - row as f32 * ENEMY_SPACING.y,
                    1.0,
                )
                .with_scale(Vec3::splat(SPRITE_SCALE)),
                Enemy { points: *pts },
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
    if digit == 0 {
        9
    } else {
        (digit - 1) as usize
    }
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

    let digits: Vec<u8> = format!("{:06}", value.min(999999))
        .chars()
        .map(|c| c.to_digit(10).unwrap() as u8)
        .collect();

    for (i, &digit) in digits.iter().enumerate() {
        let x = base_x + (i as f32) * DIGIT_SPACING;
        cmd.spawn((
            Sprite::from_atlas_image(
                sprites.number_font_texture.clone(),
                TextureAtlas {
                    layout: sprites.number_font_layout.clone(),
                    index: digit_to_atlas_index(digit),
                },
            ),
            Transform::from_xyz(x, y, 10.0).with_scale(Vec3::splat(DIGIT_SCALE)),
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
    let base_x = -screen.half_width + 30.0;
    let y = screen.half_height - 60.0;
    let digit_spacing = 22.0;

    let digits: Vec<u8> = format!("{:02}", wave.min(99))
        .chars()
        .map(|c| c.to_digit(10).unwrap() as u8)
        .collect();

    for (i, &digit) in digits.iter().enumerate() {
        let x = base_x + (i as f32) * digit_spacing;
        cmd.spawn((
            Sprite::from_atlas_image(
                sprites.number_font_texture.clone(),
                TextureAtlas {
                    layout: sprites.number_font_layout.clone(),
                    index: digit_to_atlas_index(digit),
                },
            ),
            Transform::from_xyz(x, y, 10.0).with_scale(Vec3::splat(WAVE_DIGIT_SCALE)),
            WaveDigit,
            GameEntity,
        ));
    }
}

/// Spawn a random power-up that the player doesn't have yet
pub fn spawn_powerup(
    cmd: &mut Commands,
    pos: Vec3,
    sprites: &SpriteAssets,
    powerups: &PowerUpState,
    rng: &mut SmallRng,
) {
    // Build list of power-ups the player doesn't have
    let mut available = Vec::new();
    if !powerups.rapid_fire {
        available.push(PowerUpType::RapidFire);
    }
    if !powerups.triple_shot {
        available.push(PowerUpType::TripleShot);
    }
    if !powerups.speed_boost {
        available.push(PowerUpType::SpeedBoost);
    }
    if !powerups.power_shot {
        available.push(PowerUpType::PowerShot);
    }

    // Don't spawn if player has all power-ups
    if available.is_empty() {
        return;
    }

    let kind = available[rng.random_range(0..available.len())];

    // Map power-up type to sprite index (0=circle, 1=square, 2=missile)
    let sprite_index = match kind {
        PowerUpType::RapidFire => 0,   // Circle
        PowerUpType::SpeedBoost => 0,  // Circle
        PowerUpType::PowerShot => 1,   // Square
        PowerUpType::TripleShot => 2,  // Missile
    };

    cmd.spawn((
        Sprite::from_atlas_image(
            sprites.powerup_texture.clone(),
            TextureAtlas {
                layout: sprites.powerup_layout.clone(),
                index: sprite_index,
            },
        ),
        Transform::from_translation(pos).with_scale(Vec3::splat(POWERUP_SCALE)),
        PowerUp { kind },
        GameEntity,
    ));
}

/// Spawn side boosters attached to the player
pub fn spawn_side_boosters(cmd: &mut Commands, player_id: Entity, sprites: &SpriteAssets) {
    // Left booster
    cmd.spawn((
        Sprite::from_atlas_image(
            sprites.booster_left_texture.clone(),
            TextureAtlas {
                layout: sprites.booster_left_layout.clone(),
                index: 0,
            },
        ),
        Transform::from_xyz(-10.0, -8.0, -0.1),
        ChildOf(player_id),
        SideBooster,
        AnimationIndices { first: 0, last: 1 },
        AnimationTimer(Timer::from_seconds(0.08, TimerMode::Repeating)),
    ));

    // Right booster
    cmd.spawn((
        Sprite::from_atlas_image(
            sprites.booster_right_texture.clone(),
            TextureAtlas {
                layout: sprites.booster_right_layout.clone(),
                index: 0,
            },
        ),
        Transform::from_xyz(10.0, -8.0, -0.1),
        ChildOf(player_id),
        SideBooster,
        AnimationIndices { first: 0, last: 1 },
        AnimationTimer(Timer::from_seconds(0.08, TimerMode::Repeating)),
    ));
}
