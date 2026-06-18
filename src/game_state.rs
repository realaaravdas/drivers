use bevy::prelude::*;

#[derive(States, Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum GameState {
    #[default]
    MainMenu,
    GeneratingLevel,
    Racing,
    GameOver,
}

#[derive(Resource)]
pub struct GameDifficulty {
    pub speed_multiplier: f32,
    pub ai_aggressiveness: f32,
    pub steering_sensitivity: f32,
}

impl Default for GameDifficulty {
    fn default() -> Self {
        Self {
            speed_multiplier: 1.0,
            ai_aggressiveness: 1.0,
            steering_sensitivity: 3.0,
        }
    }
}

#[derive(Component)]
pub struct RaceEntity;
