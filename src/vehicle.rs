use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use rand::RngExt;
use crate::game_state::{GameState, RaceEntity, GameDifficulty};
use crate::level_gen::LevelData;

pub struct VehiclePlugin;

impl Plugin for VehiclePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Racing), spawn_player_car)
           .add_systems(Update, (
               vehicle_update,
               spawn_exhaust_smoke,
               update_smoke_particles,
           ).run_if(in_state(GameState::Racing)));
    }
}

#[derive(Component)]
pub struct ExhaustPort;

#[derive(Component)]
pub struct SmokeParticle {
    pub timer: Timer,
    pub velocity: Vec3,
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
    difficulty: Res<GameDifficulty>,
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
        Damping { linear_damping: 0.5, angular_damping: 10.0 },
        Vehicle {
            speed: 0.0,
            max_speed: difficulty.top_speed,
            acceleration: difficulty.acceleration,
            steering_angle: 0.0,
            max_steering: 1.047, // 60 degrees in radians
            is_player: true,
        },
        Player,
        crate::game_state::LapTracker {
            current_lap: 1,
            total_laps: difficulty.laps,
            next_waypoint: 1, // 0 is start, so next is 1
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
            ExhaustPort,
        ));
    });
}

fn vehicle_update(
    time: Res<Time>,
    difficulty: Res<GameDifficulty>,
    mut query: Query<(&mut Vehicle, &mut ExternalForce, &Transform, &Velocity, Option<&Children>, Option<&mut crate::game_state::LapTracker>)>,
    mut wheel_query: Query<(&mut Transform, Option<&WheelFrontLeft>, Option<&WheelFrontRight>), Without<Vehicle>>,
    mut waypoint_materials: Query<(&crate::game_state::WaypointMarker, &mut MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    level_data: Res<crate::level_gen::LevelData>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    let dt = time.delta_secs();
    for (mut vehicle, mut force, transform, velocity, children, lap_tracker) in query.iter_mut() {
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
            let turn_torque = Vec3::Y * steering * 1000.0 * speed_factor * turn_dir;

            force.force = engine_force + drag_force + grip_force;
            force.torque = turn_torque;

            // Lap tracking logic
            if let Some(mut tracker) = lap_tracker {
                if !level_data.waypoints.is_empty() {
                    let target_wp = level_data.waypoints[tracker.next_waypoint];
                    if transform.translation.distance(target_wp) < 15.0 {
                        // Change color of passed waypoint
                        for (marker, mut mat) in &mut waypoint_materials {
                            if marker.0 == tracker.next_waypoint {
                                *mat = MeshMaterial3d(materials.add(Color::srgb(0.0, 1.0, 0.0)));
                            }
                        }

                        tracker.next_waypoint += 1;
                        if tracker.next_waypoint >= level_data.waypoints.len() {
                            tracker.next_waypoint = 0;
                            tracker.current_lap += 1;
                            // Reset waypoint colors for new lap, even if finished so we don't have weird state
                            for (marker, mut mat) in &mut waypoint_materials {
                                if marker.0 == 0 {
                                    *mat = MeshMaterial3d(materials.add(Color::srgb(1.0, 1.0, 1.0)));
                                } else {
                                    *mat = MeshMaterial3d(materials.add(Color::srgb(1.0, 0.0, 0.0)));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn spawn_exhaust_smoke(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    query: Query<&GlobalTransform, With<ExhaustPort>>,
) {
    let mut rng = rand::rng();
    
    for global_transform in query.iter() {
        let chance = 0.2; // 20% chance per frame to spawn smoke
        
        if rand::random::<f32>() < chance {
            let pos = global_transform.translation();
            let back = global_transform.up(); // Because cylinder is rotated X 90 deg, up is Z
            
            let scatter = Vec3::new(
                rng.random_range(-0.1..0.1),
                rng.random_range(0.0..0.2),
                rng.random_range(-0.1..0.1),
            );
            
            let vel = back * rng.random_range(2.0..5.0) + scatter + Vec3::Y * 2.0;

            commands.spawn((
                Mesh3d(meshes.add(Sphere::new(0.2).mesh().ico(2).unwrap())),
                MeshMaterial3d(materials.add(Color::srgba(0.5, 0.5, 0.5, 0.8))),
                Transform::from_translation(pos),
                SmokeParticle {
                    timer: Timer::from_seconds(rng.random_range(0.5..1.5), TimerMode::Once),
                    velocity: vel,
                },
                RaceEntity,
            ));
        }
    }
}

fn update_smoke_particles(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut Transform, &mut SmokeParticle)>,
) {
    let dt = time.delta_secs();
    for (entity, mut transform, mut particle) in query.iter_mut() {
        particle.timer.tick(time.delta());
        
        if particle.timer.elapsed() >= particle.timer.duration() {
            commands.entity(entity).despawn();
        } else {
            transform.translation += particle.velocity * dt;
            // Shrink as it fades
            let scale = particle.timer.fraction_remaining();
            transform.scale = Vec3::splat(scale);
        }
    }
}
