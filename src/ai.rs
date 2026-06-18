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
    pub stuck_time: f32,
    pub reversing_time: f32,
}

fn spawn_ai_cars(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    level_data: Res<LevelData>,
    difficulty: Res<GameDifficulty>,
) {
    let start_pos = level_data.start_pos;

    // Spawn 12 AI cars with an offset
    for i in 1..=12 {
        let row = (i + 1) / 2;
        let col = if i % 2 == 0 { 1.0 } else { -1.0 };
        let offset = Vec3::new(col * 4.0, 0.0, row as f32 * 8.0);
        let mut spawn_pos = start_pos + offset;
        
        let x = spawn_pos.x;
        let z = spawn_pos.z;
        let surface_y = (x / 200.0).sin() * 20.0 + (z / 150.0).cos() * 15.0 + (x * z / 10000.0).sin() * 10.0;
        spawn_pos.y = surface_y + 5.0;
        
        // 4 better, 4 same, 4 worse
        let spec_mod = if i <= 4 {
            1.1
        } else if i <= 8 {
            1.0
        } else {
            0.9
        };
        
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(2.0, 1.0, 4.0))),
            MeshMaterial3d(materials.add(Color::srgb(0.2, 0.8, 0.2))),
            Transform::from_translation(spawn_pos),
            RigidBody::Dynamic,
            Collider::cuboid(1.0, 0.5, 2.0),
            Velocity::default(),
            ExternalForce::default(),
            ExternalImpulse::default(),
            ReadMassProperties::default(),
            Ccd::enabled(),
            Damping { linear_damping: 0.5, angular_damping: 10.0 },
            Vehicle {
                speed: 0.0,
                max_speed: difficulty.top_speed * spec_mod,
                acceleration: difficulty.acceleration * spec_mod,
                steering_angle: 0.0,
                max_steering: 1.047, // 60 degrees in radians
                is_player: false,
            },
            AiDrivatar {
                current_waypoint: 1, // Start aiming at the second waypoint
                stuck_time: 0.0,
                reversing_time: 0.0,
            },
            crate::game_state::LapTracker {
                current_lap: 1,
                total_laps: difficulty.laps,
                next_waypoint: 1,
                race_start_time: 0.0,
                current_lap_start_time: 0.0,
                lap_times: Vec::new(),
                finished_time: None,
                place: 1,
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

            // Exhaust Port
            parent.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.1, 0.4))),
                MeshMaterial3d(materials.add(Color::srgb(0.3, 0.3, 0.3))),
                Transform::from_xyz(0.6, -0.2, 2.0).with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                crate::vehicle::ExhaustPort,
            ));
        });
    }
}

fn ai_update(
    time: Res<Time>,
    difficulty: Res<GameDifficulty>,
    mut query: Query<(Entity, &mut Vehicle, &mut ExternalForce, &Transform, &Velocity, &mut AiDrivatar, Option<&Children>, &mut crate::game_state::LapTracker)>,
    mut wheel_query: Query<(&mut Transform, Option<&WheelFrontLeft>, Option<&WheelFrontRight>), (Without<Vehicle>, Without<Player>)>,
    player_query: Query<&Transform, (With<Player>, Without<AiDrivatar>)>,
    level_data: Res<LevelData>,
) {
    let dt = time.delta_secs();
    let player_transform = player_query.iter().next();
    
    for (_entity, mut vehicle, mut force, transform, velocity, mut ai, children, mut tracker) in query.iter_mut() {
        if level_data.waypoints.is_empty() { continue; }
        
        let target_wp = level_data.waypoints[tracker.next_waypoint];
        
        // Lap and Waypoint logic
        if transform.translation.distance(target_wp) < 15.0 {
            tracker.next_waypoint += 1;
            if tracker.next_waypoint >= level_data.waypoints.len() {
                tracker.next_waypoint = 0;
                tracker.current_lap += 1;
            }
            // keep ai current_waypoint synced
            ai.current_waypoint = tracker.next_waypoint;
        }

        let target_wp = level_data.waypoints[ai.current_waypoint];
        let mut target_pos = target_wp;
        let right: Vec3 = transform.right().into();
        let forward: Vec3 = transform.forward().into();

        // Aggressive AI: Blocking behavior
        if let Some(p_transform) = player_transform {
            let to_player = p_transform.translation - transform.translation;
            let dist = to_player.length();
            
            if dist < 40.0 * difficulty.ai_aggressiveness {
                let is_behind = forward.dot(to_player) < 0.0;
                
                if is_behind {
                    // Player is behind, try to block by swerving into their lane
                    let lat_dist = right.dot(to_player);
                    // Shift target position sideways in the direction of the player
                    let block_shift = right * lat_dist.clamp(-15.0, 15.0) * 0.8 * difficulty.ai_aggressiveness;
                    target_pos += block_shift;
                } else if dist < 15.0 {
                    // Player is next to us or slightly ahead, swerve slightly into them
                    target_pos = target_pos.lerp(p_transform.translation, 0.3 * difficulty.ai_aggressiveness);
                }
            }
        }

        let to_target = (target_pos - transform.translation).normalize_or_zero();

        let mut target_steering = -right.dot(to_target).clamp(-1.0, 1.0);
        
        // Determine throttle (slow down if turning sharply)
        let forward_dot = forward.dot(to_target);
        let mut throttle = 1.0 * difficulty.ai_aggressiveness;
        if forward_dot < 0.5 {
            throttle = 0.7 * difficulty.ai_aggressiveness; // Brake slightly, but don't become snails
        }

        // Check if stuck
        if velocity.linear.length() < 2.0 {
            ai.stuck_time += dt;
            if ai.stuck_time > 2.0 {
                ai.reversing_time = 1.5;
                ai.stuck_time = 0.0;
            }
        } else {
            ai.stuck_time = 0.0;
        }

        if ai.reversing_time > 0.0 {
            ai.reversing_time -= dt;
            throttle = -1.0;
            target_steering = -target_steering; // Turn opposite way to back out
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
        let turn_torque = Vec3::Y * steering * 1000.0 * speed_factor * turn_dir;

        force.force = engine_force + drag_force + grip_force;
        
        let up: Vec3 = transform.up().into();
        let mut righting_torque = Vec3::ZERO;
        
        let tilt_axis = up.cross(Vec3::Y);
        righting_torque += tilt_axis * 5000.0;
        force.force += -Vec3::Y * 500.0;

        force.torque = turn_torque + righting_torque;
    }
}
