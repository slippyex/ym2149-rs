//! Game resources

use bevy::prelude::*;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use ym2149_gist_replayer::{GistPlayer, GistSound};

use super::config::{EXTRA_LIFE_SCORE, FadePhase, MAX_HIGH_SCORES, STARTING_LIVES};
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
    pub booster_left_texture: Handle<Image>,
    pub booster_left_layout: Handle<TextureAtlasLayout>,
    pub booster_right_texture: Handle<Image>,
    pub booster_right_layout: Handle<TextureAtlasLayout>,
    pub enemy_alan_texture: Handle<Image>,
    pub enemy_alan_layout: Handle<TextureAtlasLayout>,
    pub enemy_bonbon_texture: Handle<Image>,
    pub enemy_bonbon_layout: Handle<TextureAtlasLayout>,
    pub enemy_lips_texture: Handle<Image>,
    pub enemy_lips_layout: Handle<TextureAtlasLayout>,
    pub player_bullet_texture: Handle<Image>,
    pub player_bullet_layout: Handle<TextureAtlasLayout>,
    pub triple_shot_texture: Handle<Image>,
    pub triple_shot_layout: Handle<TextureAtlasLayout>,
    pub power_shot_texture: Handle<Image>,
    pub power_shot_layout: Handle<TextureAtlasLayout>,
    pub enemy_bullet_texture: Handle<Image>,
    pub enemy_bullet_layout: Handle<TextureAtlasLayout>,
    pub explosion_texture: Handle<Image>,
    pub explosion_layout: Handle<TextureAtlasLayout>,
    pub life_icon_texture: Handle<Image>,
    pub number_font_texture: Handle<Image>,
    pub number_font_layout: Handle<TextureAtlasLayout>,
    pub powerup_texture: Handle<Image>,
    pub powerup_layout: Handle<TextureAtlasLayout>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnemyType {
    Alan,
    BonBon,
    Lips,
}

/// Font assets for arcade-style text
#[derive(Resource, Clone)]
pub struct FontAssets {
    pub arcade: Handle<Font>,
}

/// Single high score entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HighScoreEntry {
    pub name: String,
    pub score: u32,
    pub wave: u32,
}

/// Persistent high score list
#[derive(Resource, Debug, Clone, Serialize, Deserialize)]
pub struct HighScoreList {
    pub entries: Vec<HighScoreEntry>,
}

impl Default for HighScoreList {
    fn default() -> Self {
        Self {
            entries: (0..MAX_HIGH_SCORES)
                .map(|i| HighScoreEntry {
                    name: "AAA".to_string(),
                    score: 1000 * (MAX_HIGH_SCORES - i) as u32,
                    wave: 1,
                })
                .collect(),
        }
    }
}

impl HighScoreList {
    fn save_path() -> Option<PathBuf> {
        ProjectDirs::from("com", "ym2149", "space_shooter")
            .map(|dirs| dirs.data_dir().join("highscores.json"))
    }

    pub fn load() -> Self {
        Self::save_path()
            .and_then(|path| fs::read_to_string(&path).ok())
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if let Some(path) = Self::save_path() {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string_pretty(self) {
                let _ = fs::write(&path, json);
            }
        }
    }

    pub fn is_high_score(&self, score: u32) -> bool {
        score > 0
            && (self.entries.len() < MAX_HIGH_SCORES
                || self.entries.last().is_some_and(|e| score > e.score))
    }

    /// Add a score and return its position in the list (0-indexed)
    pub fn add_score(&mut self, name: String, score: u32, wave: u32) -> usize {
        let name_for_search = name.clone();
        let entry = HighScoreEntry { name, score, wave };
        self.entries.push(entry);
        self.entries.sort_by(|a, b| b.score.cmp(&a.score));
        self.entries.truncate(MAX_HIGH_SCORES);
        self.save();
        // Find the position of the newly added score
        self.entries
            .iter()
            .position(|e| e.score == score && e.name == name_for_search)
            .unwrap_or(0)
    }
}

/// Tracks the index of a newly entered high score for highlighting
#[derive(Resource, Default)]
pub struct NewHighScoreIndex(pub Option<usize>);

/// Timer for auto-cycling screens (10 seconds)
#[derive(Resource)]
pub struct ScreenCycleTimer(pub Timer);

impl Default for ScreenCycleTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(10.0, TimerMode::Once))
    }
}

/// Current name entry state
#[derive(Resource)]
pub struct NameEntryState {
    pub chars: [char; 3],
    pub position: usize,
}

impl Default for NameEntryState {
    fn default() -> Self {
        Self {
            chars: ['A', 'A', 'A'],
            position: 0,
        }
    }
}

/// Whether the game is in attract mode (cycling between title and high scores)
#[derive(Resource)]
pub struct AttractMode {
    pub active: bool,
}

impl Default for AttractMode {
    fn default() -> Self {
        Self { active: true } // Start in attract mode
    }
}

/// Screen fade state for smooth transitions
#[derive(Resource)]
pub struct ScreenFade {
    pub alpha: f32,
    pub fading_in: bool,
    pub fading_out: bool,
    pub timer: Timer,
}

impl Default for ScreenFade {
    fn default() -> Self {
        Self {
            alpha: 0.0,
            fading_in: true,
            fading_out: false,
            timer: Timer::from_seconds(0.5, TimerMode::Once),
        }
    }
}

impl ScreenFade {
    pub const FADE_DURATION: f32 = 0.5;

    pub fn start_fade_in(&mut self) {
        self.fading_in = true;
        self.fading_out = false;
        self.alpha = 0.0;
        self.timer = Timer::from_seconds(Self::FADE_DURATION, TimerMode::Once);
    }

    pub fn start_fade_out(&mut self) {
        self.fading_in = false;
        self.fading_out = true;
        self.timer = Timer::from_seconds(Self::FADE_DURATION, TimerMode::Once);
    }
}

/// Tracks active power-ups for the player
#[derive(Resource, Default)]
pub struct PowerUpState {
    pub rapid_fire: bool,
    pub triple_shot: bool,
    pub speed_boost: bool,
    pub power_shot: bool,
}

impl PowerUpState {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn has_any(&self) -> bool {
        self.rapid_fire || self.triple_shot || self.speed_boost || self.power_shot
    }
}

/// Quit confirmation dialog state
#[derive(Resource, Default)]
pub struct QuitConfirmation {
    pub showing: bool,
}

/// CLI setting: whether music is enabled
#[derive(Resource)]
pub struct MusicEnabled(pub bool);

/// Screen shake effect state
#[derive(Resource, Default)]
pub struct ScreenShake {
    pub trauma: f32,     // Current shake intensity (0.0 - 1.0)
    pub decay_rate: f32, // How fast trauma decays
}

impl ScreenShake {
    pub fn add_trauma(&mut self, amount: f32) {
        self.trauma = (self.trauma + amount).min(1.0);
        self.decay_rate = 5.0; // Default decay
    }
}

/// Combo tracking for rapid kills
#[derive(Resource, Default)]
pub struct ComboTracker {
    pub count: u32,
    pub timer: f32,
}

impl ComboTracker {
    const COMBO_WINDOW: f32 = 1.0; // Seconds to maintain combo

    pub fn add_kill(&mut self) {
        self.count += 1;
        self.timer = Self::COMBO_WINDOW;
    }

    pub fn tick(&mut self, dt: f32) {
        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.count = 0;
            }
        }
    }

    pub fn current(&self) -> u32 {
        self.count.max(1)
    }
}

/// Screen flash effect for power-up collection
#[derive(Resource, Default)]
pub struct ScreenFlash {
    pub timer: f32,
    pub color: Color,
}
