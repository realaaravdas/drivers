use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use rand::RngExt;
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

/// A racing opponent's driving personality. Each car gets a random mix, so they
/// feel like distinct human drivers rather than clones.
#[derive(Component)]
pub struct AiDrivatar {
    pub current_waypoint: usize,
    pub stuck_time: f32,
    pub reversing_time: f32,
    /// How hard they push / block / commit to corners (≈0.6 timid … 1.35 reckless).
    pub aggression: f32,
    /// How early/much they slow for corners (≈0.7 late-braker … 1.3 very careful).
    pub caution: f32,
    /// How cleanly they drive; low values make occasional mistakes (0.55 … 0.98).
    pub consistency: f32,
    /// Countdown of an in-progress mistake, and the steering wobble it applies.
    pub mistake_timer: f32,
    pub mistake_steer: f32,
    /// Slight personal racing-line bias (apex-hugging vs wide), in metres.
    pub line_bias: f32,
}

fn spawn_ai_cars(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    level_data: Res<LevelData>,
    difficulty: Res<GameDifficulty>,
) {
    let mut rng = rand::rng();

    // Spawn 12 AI cars in the starting grid behind the player (index 0 = pole).
    for i in 1..=12 {
        let spawn_tf = crate::vehicle::grid_slot(&level_data.waypoints, i);
        let spawn_pos = spawn_tf.translation;

        // Difficulty-driven pace: every car's raw spec is the difficulty mean with
        // a small random spread, so there are no fixed "tiers". Even the quickest
        // car sits only ~14% above the mean, and their personality flaws (caution,
        // mistakes, imperfect lines) mean a skilled player can beat any of them —
        // while a high difficulty still makes the whole field genuinely fast.
        let spec_mod = difficulty.ai_skill * (1.0 + rng.random_range(-0.12..0.14));

        // Distinct human-like personality per car.
        let aggression = rng.random_range(0.6..1.35);
        let caution = rng.random_range(0.7..1.3);
        let consistency = rng.random_range(0.55..0.98);
        let line_bias = rng.random_range(-4.0..4.0);

        let tail_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(0.3, 0.0, 0.0),
            emissive: Color::srgb(0.5, 0.0, 0.0).to_linear(),
            ..default()
        });

        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(2.0, 1.0, 4.0))),
            MeshMaterial3d(materials.add(Color::srgb(0.2, 0.8, 0.2))),
            spawn_tf,
            RigidBody::Dynamic,
            Collider::round_cuboid(0.9, 0.4, 1.9, 0.1),
            Velocity::default(),
            ExternalForce::default(),
            ExternalImpulse::default(),
            ReadMassProperties::default(),
            Ccd::enabled(),
            Damping { linear_damping: 0.5, angular_damping: crate::vehicle::CAR_ANGULAR_DAMPING },
            Vehicle {
                speed: 0.0,
                max_speed: difficulty.top_speed * spec_mod,
                acceleration: difficulty.acceleration * spec_mod,
                steering_angle: 0.0,
                max_steering: 1.047, // 60 degrees in radians
                is_player: false,
                throttle: 0.0,
                braking: false,
                drifting: false,
            },
            AiDrivatar {
                current_waypoint: 1, // Start aiming at the second waypoint
                stuck_time: 0.0,
                reversing_time: 0.0,
                aggression,
                caution,
                consistency,
                mistake_timer: 0.0,
                mistake_steer: 0.0,
                line_bias,
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
            (
                RaceEntity,
                crate::vehicle::TireMarks { last_mark: spawn_pos },
                crate::vehicle::CarLights { tail_mat: tail_mat.clone() },
            ),
        )).with_children(|parent| {
            crate::vehicle::build_car_visuals(parent, &mut meshes, &mut materials, Color::srgb(0.2, 0.8, 0.2), tail_mat.clone());
        });
    }
}

fn ai_update(
    time: Res<Time>,
    difficulty: Res<GameDifficulty>,
    mut query: Query<(Entity, &mut Vehicle, &mut ExternalForce, &Transform, &mut Velocity, &mut AiDrivatar, Option<&Children>, &mut crate::game_state::LapTracker)>,
    mut wheel_query: Query<(&mut Transform, Option<&WheelFrontLeft>, Option<&WheelFrontRight>), (Without<Vehicle>, Without<Player>)>,
    player_query: Query<&Transform, (With<Player>, Without<AiDrivatar>)>,
    level_data: Res<LevelData>,
    rapier: ReadRapierContext,
) {
    let dt = time.delta_secs();
    let player_transform = player_query.iter().next();
    let rapier_ctx = rapier.single().ok();
    let mut rng = rand::rng();

    for (_entity, mut vehicle, mut force, transform, mut velocity, mut ai, children, mut tracker) in query.iter_mut() {
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

        let right: Vec3 = transform.right().into();
        let forward: Vec3 = transform.forward().into();

        // Follow the actual road centreline: find the nearest point on it and aim
        // a look-ahead distance further along. This keeps the AI ON the curvy road
        // instead of cutting the chord between distant waypoints.
        let cl = &level_data.road_centerline;
        let mut nearest = 0usize;
        let mut nd = f32::MAX;
        for (idx, c) in cl.iter().enumerate() {
            let dx = c.x - transform.translation.x;
            let dz = c.z - transform.translation.z;
            let d = dx * dx + dz * dz;
            if d < nd {
                nd = d;
                nearest = idx;
            }
        }
        let look = 3 + (velocity.linear.length() * 0.1) as usize;
        let aim = cl[(nearest + look) % cl.len().max(1)];

        // Personal racing line: nudge the aim point sideways a little.
        let mut target_pos = aim + right * ai.line_bias;

        // Blocking / defending — driven by this car's aggression and the global slider.
        let effective_aggr = ai.aggression * difficulty.ai_aggressiveness;
        if let Some(p_transform) = player_transform {
            let to_player = p_transform.translation - transform.translation;
            let dist = to_player.length();

            if dist < 40.0 * effective_aggr {
                let is_behind = forward.dot(to_player) < 0.0;
                if is_behind {
                    // Player is behind — swerve to cover their line.
                    let lat_dist = right.dot(to_player);
                    target_pos += right * lat_dist.clamp(-15.0, 15.0) * 0.8 * effective_aggr;
                } else if dist < 15.0 {
                    // Side by side — lean on them a bit.
                    target_pos = target_pos.lerp(p_transform.translation, 0.3 * effective_aggr);
                }
            }
        }

        let to_target = (target_pos - transform.translation).normalize_or_zero();

        let mut target_steering = -right.dot(to_target).clamp(-1.0, 1.0);

        // Actively return to the road centre: steer toward the nearest centreline
        // point in proportion to how far off it we are. This is what makes them
        // hug the racing line instead of drifting onto the grass.
        let center = cl[nearest];
        let lateral = right.dot(Vec3::new(
            center.x - transform.translation.x,
            0.0,
            center.z - transform.translation.z,
        ));
        target_steering = (target_steering - lateral * 0.06).clamp(-1.0, 1.0);

        // Speed management by UPCOMING curvature so cars actually slow for corners
        // and don't run wide off the road. `curve` is 0 on a straight, larger in a
        // tight bend. Aggressive drivers carry more speed; cautious ones slow more.
        let cn = cl.len().max(1);
        let d_near = (cl[(nearest + 4) % cn] - cl[(nearest + 2) % cn]).normalize_or_zero();
        let d_far = (cl[(nearest + 16) % cn] - cl[(nearest + 11) % cn]).normalize_or_zero();
        let curve = (1.0 - d_near.dot(d_far).clamp(-1.0, 1.0)).max(0.0);
        let speed = velocity.linear.length();
        let corner_speed = (16.0 + 34.0 * ai.aggression) / (1.0 + curve * 5.0 * ai.caution);
        let mut throttle = if speed < corner_speed { 1.0 } else { 0.0 };
        let mut braking = speed > corner_speed * 1.15;
        let mut drifting = curve > 0.5 && speed > 16.0 && ai.aggression > 1.05;

        // Obstacle avoidance: feeler rays detect buildings and other cars ahead
        // (terrain has no collider, so rays only hit real obstacles). We steer away
        // from the closer side and back off the throttle when something's dead ahead.
        if let Some(ctx) = &rapier_ctx {
            let fwd_speed = velocity.linear.dot(forward);
            let feel = 7.0 + fwd_speed.max(0.0) * 0.35;
            let origin = transform.translation + forward * 2.6 + Vec3::Y * 0.2;
            let left_dir = (forward - right * 0.6).normalize_or_zero();
            let right_dir = (forward + right * 0.6).normalize_or_zero();
            let ray = |dir: Vec3| ctx.cast_ray(origin, dir, feel, false, QueryFilter::default()).map(|(_, t)| t).unwrap_or(feel);
            let (dl, dr, dc) = (ray(left_dir), ray(right_dir), ray(forward));
            // avoid > 0 → obstacle nearer on the right → steer left (positive), and vice versa.
            let avoid = (1.0 - dr / feel) - (1.0 - dl / feel);
            target_steering = (target_steering + avoid * 2.2).clamp(-1.0, 1.0);
            if dc < feel * 0.55 {
                throttle *= 0.4;
            }
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

        // Occasional human mistakes — more likely for low-consistency drivers. This
        // is what lets a skilled player pick any of them off.
        if ai.mistake_timer > 0.0 {
            ai.mistake_timer -= dt;
            target_steering = (target_steering + ai.mistake_steer).clamp(-1.0, 1.0);
            throttle *= 0.8;
        } else if rand::random::<f32>() < (1.0 - ai.consistency) * 0.004 {
            ai.mistake_timer = rng.random_range(0.4..1.1);
            ai.mistake_steer = rng.random_range(-0.45..0.45);
        }

        if ai.reversing_time > 0.0 {
            ai.reversing_time -= dt;
            throttle = -1.0;
            target_steering = -target_steering; // Turn opposite way to back out
            braking = false;
            drifting = false;
        }

        vehicle.braking = braking;
        vehicle.drifting = drifting;
        if braking {
            throttle = 0.0;
        }

        // Smooth steering (snappier than before so they react through corners).
        vehicle.steering_angle += (target_steering * vehicle.max_steering - vehicle.steering_angle) * 0.22;

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

        let mut engine_force = forward * throttle * vehicle.acceleration;
        if vehicle.braking {
            engine_force += -forward * current_fwd_vel * 13.0; // strong brakes, same as player
        }

        let drag_force = -forward * current_fwd_vel * 1.0;
        let grip_factor = if vehicle.drifting { 6.0 } else { 39.0 };
        let grip_force = -right * current_lat_vel * grip_factor;

        force.force = engine_force + drag_force + grip_force;
        force.torque = Vec3::ZERO;

        // Realistic, speed-limited turning (same model as the player).
        let target_yaw = crate::vehicle::steering_yaw_rate(current_fwd_vel, vehicle.steering_angle, vehicle.max_speed, vehicle.drifting);
        velocity.angular.y = target_yaw * (1.0 + crate::vehicle::CAR_ANGULAR_DAMPING * dt);

        // Smoothly follow and align to the analytic terrain (no ground collider).
        crate::vehicle::apply_terrain_follow(transform, &mut velocity, &mut force, dt);
    }
}
