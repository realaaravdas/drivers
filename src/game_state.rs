use bevy::prelude::*;

#[derive(States, Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum GameState {
    #[default]
    Splash,
    MainMenu,
    Loading,
    GeneratingLevel,
    Racing,
    Scoreboard,
    PostRace,
}

#[derive(Resource)]
pub struct GameDifficulty {
    pub ai_aggressiveness: f32,
    pub steering_sensitivity: f32,
    pub top_speed: f32,
    pub acceleration: f32,
    pub laps: u32,
}

impl Default for GameDifficulty {
    fn default() -> Self {
        Self {
            ai_aggressiveness: 1.0,
            steering_sensitivity: 3.0,
            top_speed: 120.0,
            acceleration: 500.0,
            laps: 3,
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
    pub race_start_time: f32,
    pub current_lap_start_time: f32,
    pub lap_times: Vec<f32>,
    pub finished_time: Option<f32>,
    pub place: usize, // e.g. 1st, 2nd
}

#[derive(Component)]
pub struct WaypointMarker(pub usize);
