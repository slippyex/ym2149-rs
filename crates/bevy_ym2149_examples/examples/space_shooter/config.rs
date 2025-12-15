//! Game configuration constants, states, and messages

use bevy::prelude::*;

// === Gameplay Constants ===
pub const PLAYER_SPEED: f32 = 400.0;
pub const PLAYER_SIZE: Vec2 = Vec2::new(48.0, 48.0);
pub const BULLET_SPEED: f32 = 600.0;
pub const BULLET_SIZE: Vec2 = Vec2::new(24.0, 24.0);
pub const ENEMY_SIZE: Vec2 = Vec2::new(48.0, 48.0);
pub const ENEMY_BULLET_SPEED: f32 = 300.0;
pub const ENEMY_SPACING: Vec2 = Vec2::new(56.0, 52.0);
pub const STARTING_LIVES: u32 = 3;
pub const EXTRA_LIFE_SCORE: u32 = 3000;
pub const FADE_DURATION: f32 = 2.0;
pub const SPRITE_SCALE: f32 = 3.0;
pub const ANIM_FPS: f32 = 8.0;

// === Power-up Constants ===
pub const POWERUP_DROP_CHANCE: f32 = 0.05; // 5% chance to drop
pub const POWERUP_SPEED: f32 = 100.0;
pub const POWERUP_SIZE: Vec2 = Vec2::new(32.0, 32.0);
pub const POWERUP_SCALE: f32 = 2.0;
pub const RAPID_FIRE_RATE: f32 = 0.12; // faster than normal 0.25
pub const SPEED_BOOST_MULT: f32 = 1.5;
pub const TRIPLE_SHOT_SPREAD: f32 = 15.0; // degrees

// === UI Constants ===
pub const LIFE_ICON_SCALE: f32 = 2.0;
pub const LIFE_ICON_SPACING: f32 = 36.0;
pub const DIGIT_SCALE: f32 = 3.0;
pub const DIGIT_SPACING: f32 = 26.0;
pub const WAVE_DIGIT_SCALE: f32 = 2.5;

// === VFX Constants ===
pub const SHAKE_INTENSITY: f32 = 12.0; // Max shake offset in pixels
pub const SHAKE_TRAUMA_EXPLOSION: f32 = 0.5; // Trauma from enemy explosion
pub const SHAKE_TRAUMA_PLAYER_HIT: f32 = 1.0; // Trauma from player death
pub const SCORE_POPUP_DURATION: f32 = 0.8; // Seconds for popup to fade
pub const SCORE_POPUP_RISE: f32 = 40.0; // Pixels to rise

// === High Score Constants ===
pub const MAX_HIGH_SCORES: usize = 10;

// === Game States ===
#[derive(States, Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum GameState {
    #[default]
    TitleScreen,
    Playing,
    NameEntry,
    HighScores,
    PowerUpsScreen,
    EnemyScoresScreen,
    GameOver,
}

// === Messages ===
#[derive(bevy::ecs::message::Message)]
pub struct PlaySfxMsg(pub SfxType);

#[derive(bevy::ecs::message::Message)]
pub struct WaveCompleteMsg;

#[derive(bevy::ecs::message::Message)]
pub struct PlayerHitMsg;

#[derive(bevy::ecs::message::Message)]
pub struct EnemyKilledMsg(pub u32);

#[derive(bevy::ecs::message::Message)]
pub struct ExtraLifeMsg;

#[derive(bevy::ecs::message::Message)]
pub struct PowerUpCollectedMsg(pub super::components::PowerUpType);

#[derive(Clone, Copy)]
pub enum SfxType {
    Laser,
    Explode,
    Death,
    PowerUp,
}

// === Music Fade Phase ===
#[derive(Default, Clone, Copy, PartialEq)]
pub enum FadePhase {
    #[default]
    Idle,
    FadeOut,
    FadeIn,
}

// === System Sets ===
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameSet {
    Input,
    Movement,
    Collision,
    Spawn,
    Cleanup,
}
