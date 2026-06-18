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
    pub ai_aggressiveness: f32,
    pub steering_sensitivity: f32,
    pub top_speed: f32,
    pub acceleration: f32,
}

impl Default for GameDifficulty {
    fn default() -> Self {
        Self {
            ai_aggressiveness: 1.0,
            steering_sensitivity: 3.0,
            top_speed: 120.0,
            acceleration: 500.0,
        }
    }
}

#[derive(Component)]
pub struct RaceEntity;

#[derive(Component)]
pub struct LapTracker {
    pub current_lap: u32,
    pub total_laps: u32,
    pub next_waypoint: usize,
}

#[derive(Component)]
pub struct WaypointMarker(pub usize);
