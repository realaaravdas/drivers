use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use crate::game_state::GameState;
use crate::level_gen::LevelData;

pub struct VehiclePlugin;

impl Plugin for VehiclePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Racing), spawn_player_car)
           .add_systems(Update, vehicle_update.run_if(in_state(GameState::Racing)));
    }
}

#[derive(Component)]
pub struct Vehicle {
    pub speed: f32,
    pub max_speed: f32,
    pub acceleration: f32,
    pub steering_angle: f32,
    pub max_steering: f32,
    pub is_player: bool,
}

#[derive(Component)]
pub struct Player;

fn spawn_player_car(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    level_data: Res<LevelData>,
) {
    let start_pos = level_data.start_pos;

    // Car chassis
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(2.0, 1.0, 4.0))),
        MeshMaterial3d(materials.add(Color::srgb(0.9, 0.1, 0.1))),
        Transform::from_translation(start_pos),
        RigidBody::Dynamic,
        Collider::cuboid(1.0, 0.5, 2.0),
        LockedAxes::ROTATION_LOCKED_X | LockedAxes::ROTATION_LOCKED_Z,
        Velocity::default(),
        ExternalForce::default(),
        ExternalImpulse::default(),
        ReadMassProperties::default(),
        Damping { linear_damping: 0.5, angular_damping: 5.0 },
        Vehicle {
            speed: 0.0,
            max_speed: 40.0,
            acceleration: 3000.0,
            steering_angle: 0.0,
            max_steering: 0.5, // radians
            is_player: true,
        },
        Player,
    ));
}

fn vehicle_update(
    mut query: Query<(&mut Vehicle, &mut ExternalForce, &Transform, &Velocity)>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    for (mut vehicle, mut force, transform, velocity) in query.iter_mut() {
        if vehicle.is_player {
            let mut throttle = 0.0;
            let mut steering = 0.0;

            if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
                throttle += 1.0;
            }
            if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
                throttle -= 1.0;
            }
            if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
                steering += 1.0;
            }
            if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
                steering -= 1.0;
            }

            vehicle.steering_angle = steering * vehicle.max_steering;

            let forward: Vec3 = transform.forward().into();
            let right: Vec3 = transform.right().into();
            
            // Simple arcade-sim style: apply forward force based on throttle
            let engine_force = forward * throttle * vehicle.acceleration;
            
            // Apply lateral friction (grip)
            let current_lat_vel = velocity.linear.dot(right);
            let grip_force = -right * current_lat_vel * 150.0; // Magic number for grip

            // Turn the car model manually or apply torque
            let turn_torque = Vec3::Y * steering * 2000.0;

            force.force = engine_force + grip_force;
            force.torque = turn_torque;
        }
    }
}
