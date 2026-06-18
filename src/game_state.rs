use bevy::prelude::*;

#[derive(States, Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum GameState {
    #[default]
    MainMenu,
    GeneratingLevel,
    Racing,
    GameOver,
}

#[derive(Resource, Default)]
pub struct GameDifficulty {
    pub speed_multiplier: f32,
    pub ai_aggressiveness: f32,
}
