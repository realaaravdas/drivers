use bevy::prelude::*;
use crate::game_state::GameState;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::MainMenu), setup_camera)
           .add_systems(Update, camera_follow.run_if(in_state(GameState::Racing)));
    }
}

#[derive(Component)]
pub struct MainCamera;

fn setup_camera(mut commands: Commands, query: Query<Entity, With<MainCamera>>) {
    if query.is_empty() {
        commands.spawn((
            Camera3d::default(),
            Transform::from_xyz(0.0, 5.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
            MainCamera,
        ));
    }
}

fn camera_follow(
    mut camera_query: Query<&mut Transform, (With<MainCamera>, Without<crate::vehicle::Player>)>,
    player_query: Query<&Transform, With<crate::vehicle::Player>>,
) {
    for mut camera_transform in camera_query.iter_mut() {
        for player_transform in player_query.iter() {
            let target_pos = player_transform.translation + player_transform.back() * 10.0 + Vec3::Y * 5.0;
            camera_transform.translation = camera_transform.translation.lerp(target_pos, 0.1);
            camera_transform.look_at(player_transform.translation + Vec3::Y * 2.0, Vec3::Y);
        }
    }
}
