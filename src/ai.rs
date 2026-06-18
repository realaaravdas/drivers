use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use crate::game_state::{GameState, RaceEntity, GameDifficulty};
use crate::level_gen::LevelData;
use crate::vehicle::Vehicle;

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
                max_speed: 40.0, // Match player max speed
                acceleration: 200.0, // Match player acceleration
                steering_angle: 0.0,
                max_steering: 1.0,
                is_player: false,
            },
            AiDrivatar {
                current_waypoint: 1, // Start aiming at the second waypoint
            },
            RaceEntity,
        ));
    }
}

fn ai_update(
    difficulty: Res<GameDifficulty>,
    mut query: Query<(&mut Vehicle, &mut ExternalForce, &Transform, &Velocity, &mut AiDrivatar)>,
    level_data: Res<LevelData>,
) {
    for (mut vehicle, mut force, transform, velocity, mut ai) in query.iter_mut() {
        if level_data.waypoints.is_empty() { continue; }
        
        let target_wp = level_data.waypoints[ai.current_waypoint];
        
        // If close enough to waypoint, go to next
        if transform.translation.distance(target_wp) < 15.0 {
            ai.current_waypoint = (ai.current_waypoint + 1) % level_data.waypoints.len();
        }

        let target_wp = level_data.waypoints[ai.current_waypoint];
        let to_target = (target_wp - transform.translation).normalize_or_zero();
        
        let forward: Vec3 = transform.forward().into();
        let right: Vec3 = transform.right().into();

        // Calculate steering based on dot product of right vector and direction to target. 
        // Need to negate it because positive steering rotates left (towards +Z from +X), 
        // while positive dot product means target is to the right (+X).
        let steering = -right.dot(to_target).clamp(-1.0, 1.0);
        
        // Determine throttle (slow down if turning sharply)
        let forward_dot = forward.dot(to_target);
        let mut throttle = 1.0 * difficulty.ai_aggressiveness;
        if forward_dot < 0.5 {
            throttle = 0.7 * difficulty.ai_aggressiveness; // Brake slightly, but don't become snails
        }

        vehicle.steering_angle = steering * vehicle.max_steering;

        let current_fwd_vel = velocity.linear.dot(forward);
        let current_lat_vel = velocity.linear.dot(right);

        let engine_force = forward * throttle * vehicle.acceleration;
        
        let drag_force = -forward * current_fwd_vel * 2.0;
        let grip_force = -right * current_lat_vel * 40.0; 

        let speed_factor = (current_fwd_vel.abs() / 5.0).clamp(0.0, 1.0);
        let turn_dir = if current_fwd_vel < -0.1 { -1.0 } else { 1.0 };
        let turn_torque = Vec3::Y * steering * 300.0 * speed_factor * turn_dir;

        force.force = engine_force + drag_force + grip_force;
        force.torque = turn_torque;
    }
}
