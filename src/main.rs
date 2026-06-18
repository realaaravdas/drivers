mod game_state;
mod ui;
mod level_gen;
mod vehicle;
mod ai;
mod camera;
mod hud;

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use game_state::{GameState, GameDifficulty, RaceEntity};

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.53, 0.81, 0.92))) // Sky Blue
        .add_plugins(DefaultPlugins)
        .add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
        // .add_plugins(RapierDebugRenderPlugin::default()) // Uncomment to debug physics
        .init_state::<GameState>()
        .insert_resource(GameDifficulty::default())
        .add_plugins(ui::UiPlugin)
        .add_plugins(level_gen::LevelGenPlugin)
        .add_plugins(vehicle::VehiclePlugin)
        .add_plugins(ai::AiPlugin)
        .add_plugins(camera::CameraPlugin)
        .add_plugins(hud::HudPlugin)
        .add_systems(Startup, setup_environment)
        .add_systems(Update, check_exit_to_menu.run_if(in_state(GameState::Racing)))
        .add_systems(OnExit(GameState::Racing), cleanup_racing)
        .run();
}

fn setup_environment(mut commands: Commands) {
    // We will setup global lighting here that persists or is recreated.
    // Let's create a cartoon/comic style lighting.
    commands.spawn((
        DirectionalLight {
            shadows_enabled: true,
            illuminance: 30000.0,
            color: Color::srgb(1.0, 0.98, 0.9), // Slightly warm sunlight
            ..default()
        },
        // Better angle for casting terrain shadows so elevation is visible
        Transform::from_xyz(20.0, 30.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // Secondary light to simulate ambient light and fill in shadows
    commands.spawn((
        DirectionalLight {
            shadows_enabled: false,
            illuminance: 5000.0,
            color: Color::srgb(0.6, 0.8, 1.0),
            ..default()
        },
        Transform::from_xyz(-10.0, -10.0, -10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn check_exit_to_menu(
    keys: Res<ButtonInput<KeyCode>>,
    mut game_state: ResMut<NextState<GameState>>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        game_state.set(GameState::MainMenu);
    }
}

fn cleanup_racing(
    mut commands: Commands,
    query: Query<Entity, With<RaceEntity>>,
) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}
