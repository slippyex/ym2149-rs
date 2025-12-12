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

// === UI Constants ===
pub const LIFE_ICON_SCALE: f32 = 2.0;
pub const LIFE_ICON_SPACING: f32 = 36.0;
pub const DIGIT_SCALE: f32 = 3.0;
pub const DIGIT_SPACING: f32 = 26.0;
pub const WAVE_DIGIT_SCALE: f32 = 2.5;

// === Game States ===
#[derive(States, Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum GameState {
    #[default]
    TitleScreen,
    Playing,
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

#[derive(Clone, Copy)]
pub enum SfxType {
    Laser,
    Explode,
    Death,
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
