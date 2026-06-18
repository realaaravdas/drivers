use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use crate::game_state::{GameState, RaceEntity, GameDifficulty};
use crate::level_gen::LevelData;
use crate::vehicle::{Vehicle, Player, WheelFrontLeft, WheelFrontRight};

pub struct AiPlugin;

impl Plugin for AiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Racing), spawn_ai_cars)
           .add_systems(Update, ai_update.run_if(in_state(GameState::Racing)));
    }
}

#[derive(Component)]
pub struct AiDrivatar {
    pub current_waypoint: usize,
}

fn spawn_ai_cars(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    level_data: Res<LevelData>,
) {
    let start_pos = level_data.start_pos;

    // Spawn 3 AI cars with an offset
    for i in 1..=3 {
        let offset = Vec3::new(i as f32 * 4.0, 0.0, i as f32 * 4.0);
        
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(2.0, 1.0, 4.0))),
            MeshMaterial3d(materials.add(Color::srgb(0.2, 0.8, 0.2))),
            Transform::from_translation(start_pos + offset),
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
                acceleration: 80.0,
                steering_angle: 0.0,
                max_steering: 1.047, // 60 degrees in radians
                is_player: false,
            },
            AiDrivatar {
                current_waypoint: 1, // Start aiming at the second waypoint
            },
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
}

fn ai_update(
    difficulty: Res<GameDifficulty>,
    mut query: Query<(Entity, &mut Vehicle, &mut ExternalForce, &Transform, &Velocity, &mut AiDrivatar, Option<&Children>)>,
    mut wheel_query: Query<(&mut Transform, Option<&WheelFrontLeft>, Option<&WheelFrontRight>), (Without<Vehicle>, Without<Player>)>,
    player_query: Query<&Transform, (With<Player>, Without<AiDrivatar>)>,
    level_data: Res<LevelData>,
) {
    let player_transform = player_query.iter().next();
    
    for (entity, mut vehicle, mut force, transform, velocity, mut ai, children) in query.iter_mut() {
        if level_data.waypoints.is_empty() { continue; }
        
        let target_wp = level_data.waypoints[ai.current_waypoint];
        
        // If close enough to waypoint, go to next
        if transform.translation.distance(target_wp) < 15.0 {
            ai.current_waypoint = (ai.current_waypoint + 1) % level_data.waypoints.len();
        }

        let target_wp = level_data.waypoints[ai.current_waypoint];
        let mut target_pos = target_wp;

        // Aggressive AI: if player is nearby, aim for the player to cut them off
        if let Some(p_transform) = player_transform {
            if transform.translation.distance(p_transform.translation) < 30.0 * difficulty.ai_aggressiveness {
                target_pos = p_transform.translation;
            }
        }

        let to_target = (target_pos - transform.translation).normalize_or_zero();
        
        let forward: Vec3 = transform.forward().into();
        let right: Vec3 = transform.right().into();

        // Calculate steering based on dot product of right vector and direction to target. 
        // Need to negate it because positive steering rotates left (towards +Z from +X), 
        // while positive dot product means target is to the right (+X).
        let target_steering = -right.dot(to_target).clamp(-1.0, 1.0);
        
        // Determine throttle (slow down if turning sharply)
        let forward_dot = forward.dot(to_target);
        let mut throttle = 1.0 * difficulty.ai_aggressiveness;
        if forward_dot < 0.5 {
            throttle = 0.7 * difficulty.ai_aggressiveness; // Brake slightly, but don't become snails
        }

        // Smooth steering
        vehicle.steering_angle += (target_steering * vehicle.max_steering - vehicle.steering_angle) * 0.1;
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

        let current_fwd_vel = velocity.linear.dot(forward);
        let current_lat_vel = velocity.linear.dot(right);

        let engine_force = forward * throttle * vehicle.acceleration;
        
        let drag_force = -forward * current_fwd_vel * 1.0; // Lower drag
        let grip_force = -right * current_lat_vel * 40.0; 

        let speed_factor = (current_fwd_vel.abs() / 5.0).clamp(0.0, 1.0);
        let turn_dir = if current_fwd_vel < -0.1 { -1.0 } else { 1.0 };
        let turn_torque = Vec3::Y * steering * 300.0 * speed_factor * turn_dir;

        force.force = engine_force + drag_force + grip_force;
        force.torque = turn_torque;
    }
}
