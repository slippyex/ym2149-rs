//! Game resources

use bevy::prelude::*;
use std::sync::{Arc, Mutex};
use ym2149_gist_replayer::{GistPlayer, GistSound};

use super::config::{EXTRA_LIFE_SCORE, FadePhase, STARTING_LIVES};
use super::crt::CrtMaterial;

/// Cached window dimensions - updated on resize
#[derive(Resource)]
pub struct ScreenSize {
    pub width: f32,
    pub height: f32,
    pub half_width: f32,
    pub half_height: f32,
}

impl ScreenSize {
    pub fn from_window(w: &Window) -> Self {
        Self {
            width: w.width(),
            height: w.height(),
            half_width: w.width() / 2.0,
            half_height: w.height() / 2.0,
        }
    }
}

#[derive(Resource)]
pub struct GameData {
    pub score: u32,
    pub high_score: u32,
    pub lives: u32,
    pub wave: u32,
    pub shoot_timer: Timer,
    pub enemy_shoot_timer: Timer,
    pub enemy_direction: f32,
    pub dive_timer: Timer,
    pub next_extra_life: u32,
}

impl GameData {
    pub fn new() -> Self {
        Self {
            lives: STARTING_LIVES,
            wave: 1,
            enemy_direction: 1.0,
            next_extra_life: EXTRA_LIFE_SCORE,
            shoot_timer: Timer::from_seconds(0.0, TimerMode::Once),
            enemy_shoot_timer: Timer::from_seconds(1.5, TimerMode::Once),
            dive_timer: Timer::from_seconds(3.0, TimerMode::Once),
            score: 0,
            high_score: 0,
        }
    }

    pub fn reset(&mut self) {
        let hs = self.high_score;
        *self = Self::new();
        self.high_score = hs;
    }

    pub fn dive_interval(&self) -> f32 {
        (3.0 - (self.wave as f32 - 1.0) * 0.2).max(0.8)
    }

    pub fn max_divers(&self) -> usize {
        (1 + self.wave as usize / 2).min(5)
    }

    pub fn enemy_shoot_rate(&self) -> f32 {
        (1.5 - (self.wave as f32 - 1.0) * 0.1).max(0.5)
    }
}

impl Default for GameData {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Resource, Default)]
pub struct MusicFade {
    pub target_subsong: Option<usize>,
    pub phase: FadePhase,
    pub timer: f32,
}

#[derive(Resource)]
pub struct CrtState {
    pub enabled: bool,
}

#[derive(Resource)]
pub struct CrtMaterialHandle(pub Handle<CrtMaterial>);

#[derive(Resource)]
pub struct SceneRenderTarget(pub Handle<Image>);

#[derive(Resource)]
pub struct Sfx {
    pub laser: GistSound,
    pub explode: GistSound,
    pub death: GistSound,
}

#[derive(Resource, Clone)]
pub struct GistRes(pub Arc<Mutex<GistPlayer>>);

/// All sprite sheet assets for the game
#[derive(Resource)]
pub struct SpriteAssets {
    pub player_texture: Handle<Image>,
    pub player_layout: Handle<TextureAtlasLayout>,
    pub booster_texture: Handle<Image>,
    pub booster_layout: Handle<TextureAtlasLayout>,
    pub enemy_alan_texture: Handle<Image>,
    pub enemy_alan_layout: Handle<TextureAtlasLayout>,
    pub enemy_bonbon_texture: Handle<Image>,
    pub enemy_bonbon_layout: Handle<TextureAtlasLayout>,
    pub enemy_lips_texture: Handle<Image>,
    pub enemy_lips_layout: Handle<TextureAtlasLayout>,
    pub player_bullet_texture: Handle<Image>,
    pub player_bullet_layout: Handle<TextureAtlasLayout>,
    pub enemy_bullet_texture: Handle<Image>,
    pub enemy_bullet_layout: Handle<TextureAtlasLayout>,
    pub explosion_texture: Handle<Image>,
    pub explosion_layout: Handle<TextureAtlasLayout>,
    pub life_icon_texture: Handle<Image>,
    pub number_font_texture: Handle<Image>,
    pub number_font_layout: Handle<TextureAtlasLayout>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnemyType {
    Alan,
    BonBon,
    Lips,
}
