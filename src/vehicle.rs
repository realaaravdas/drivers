use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use crate::game_state::{GameState, RaceEntity, GameDifficulty};
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

#[derive(Component)]
pub struct WheelFrontLeft;

#[derive(Component)]
pub struct WheelFrontRight;

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
        Damping { linear_damping: 0.5, angular_damping: 2.0 },
        Vehicle {
            speed: 0.0,
            max_speed: 60.0,
            acceleration: 80.0, // Lower acceleration for inertia feel
            steering_angle: 0.0,
            max_steering: 1.047, // 60 degrees in radians
            is_player: true,
        },
        Player,
        RaceEntity,
    )).with_children(|parent| {
        // Add Wheels
        let wheel_mesh = meshes.add(Cylinder::new(0.4, 0.2));
        let wheel_mat = materials.add(Color::srgb(0.1, 0.1, 0.1));

        // Front Left
        parent.spawn((
            Mesh3d(wheel_mesh.clone()),
            MeshMaterial3d(wheel_mat.clone()),
            Transform::from_xyz(-1.2, -0.3, -1.5).with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
            WheelFrontLeft,
        ));
        // Front Right
        parent.spawn((
            Mesh3d(wheel_mesh.clone()),
            MeshMaterial3d(wheel_mat.clone()),
            Transform::from_xyz(1.2, -0.3, -1.5).with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
            WheelFrontRight,
        ));
        // Back Left
        parent.spawn((
            Mesh3d(wheel_mesh.clone()),
            MeshMaterial3d(wheel_mat.clone()),
            Transform::from_xyz(-1.2, -0.3, 1.5).with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
        ));
        // Back Right
        parent.spawn((
            Mesh3d(wheel_mesh.clone()),
            MeshMaterial3d(wheel_mat.clone()),
            Transform::from_xyz(1.2, -0.3, 1.5).with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
        ));
    });
}

fn vehicle_update(
    time: Res<Time>,
    difficulty: Res<GameDifficulty>,
    mut query: Query<(&mut Vehicle, &mut ExternalForce, &Transform, &Velocity, Option<&Children>)>,
    mut wheel_query: Query<(&mut Transform, Option<&WheelFrontLeft>, Option<&WheelFrontRight>), Without<Vehicle>>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    let dt = time.delta_secs();
    for (mut vehicle, mut force, transform, velocity, children) in query.iter_mut() {
        if vehicle.is_player {
            let mut throttle = 0.0;
            let mut target_steering = 0.0;
            let mut braking = false;
            let mut drifting = false;

            if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
                throttle += 1.0;
            }
            if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
                throttle -= 1.0;
            }
            if keys.pressed(KeyCode::Space) {
                braking = true;
                throttle = 0.0;
            }
            if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
                drifting = true;
            }
            if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
                target_steering += 1.0;
            }
            if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
                target_steering -= 1.0;
            }

            let steering_speed = difficulty.steering_sensitivity; // How fast the wheel turns
            let return_speed = difficulty.steering_sensitivity * 1.5; // How fast it returns to center
            
            let step = if target_steering == 0.0 { return_speed * dt } else { steering_speed * dt };
            let target_angle = target_steering * vehicle.max_steering;
            let diff = target_angle - vehicle.steering_angle;
            
            if diff.abs() <= step {
                vehicle.steering_angle = target_angle;
            } else {
                vehicle.steering_angle += diff.signum() * step;
            }

            let steering = vehicle.steering_angle / vehicle.max_steering;

            // Visual wheel steering
            if let Some(children) = children {
                for child in children.iter() {
                    let child_entity = child;
                    if let Ok((mut w_transform, fl, fr)) = wheel_query.get_mut(child_entity) {
                        if fl.is_some() || fr.is_some() {
                            w_transform.rotation = Quat::from_rotation_y(vehicle.steering_angle) * Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
                        }
                    }
                }
            }

            let forward: Vec3 = transform.forward().into();
            let right: Vec3 = transform.right().into();
            
            let current_fwd_vel = velocity.linear.dot(forward);
            let current_lat_vel = velocity.linear.dot(right);

            // Engine force (inertia build up through lower acceleration)
            let mut engine_force = forward * throttle * vehicle.acceleration;
            
            // Braking
            if braking {
                let brake_force = -forward * current_fwd_vel * 5.0; // Strong stop
                engine_force += brake_force;
            }

            // Drag
            let drag_force = -forward * current_fwd_vel * 1.0; // Reduced drag for more coasting/inertia

            // Lateral friction (grip) - drifting reduces this!
            let mut grip_factor = 40.0;
            if drifting {
                grip_factor = 10.0; // Lose grip, slide!
            }
            let grip_force = -right * current_lat_vel * grip_factor; 

            // Turn torque - cars only turn effectively when moving
            let speed_factor = (current_fwd_vel.abs() / 5.0).clamp(0.0, 1.0);
            // Reverse steering if going backwards
            let turn_dir = if current_fwd_vel < -0.1 { -1.0 } else { 1.0 };
            let turn_torque = Vec3::Y * steering * 300.0 * speed_factor * turn_dir;

            force.force = engine_force + drag_force + grip_force;
            force.torque = turn_torque;
        }
    }
}
