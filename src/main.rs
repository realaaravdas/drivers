mod game_state;
mod ui;
mod level_gen;
mod vehicle;
mod ai;
mod camera;

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use game_state::{GameState, GameDifficulty, RaceEntity};

fn main() {
    App::new()
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
            color: Color::WHITE,
            ..default()
        },
        Transform::from_xyz(10.0, 20.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // Ambient light is now a component or different, just relying on directional light for now
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
