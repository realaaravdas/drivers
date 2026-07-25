use crate::game_state::{GameDifficulty, GameState, RaceEntity};
use crate::level_gen::LevelData;
use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use rand::RngExt;

pub struct VehiclePlugin;

impl Plugin for VehiclePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(GameState::Racing),
            (spawn_player_car, init_gate_materials, init_tire_assets),
        )
        .add_systems(
            Update,
            (
                vehicle_update,
                spawn_exhaust_smoke,
                update_smoke_particles,
                tire_effects,
                update_skid_marks,
                update_gate_colors,
            )
                .run_if(in_state(GameState::Racing)),
        );
    }
}

#[derive(Resource)]
struct GateMaterials {
    red: Handle<StandardMaterial>,
    green: Handle<StandardMaterial>,
    orange: Handle<StandardMaterial>,
    yellow: Handle<StandardMaterial>,
}

fn init_gate_materials(mut commands: Commands, mut materials: ResMut<Assets<StandardMaterial>>) {
    commands.insert_resource(GateMaterials {
        red: materials.add(Color::srgb(1.0, 0.0, 0.0)),
        green: materials.add(Color::srgb(0.0, 0.9, 0.1)),
        orange: materials.add(Color::srgb(1.0, 0.45, 0.0)),
        yellow: materials.add(Color::srgb(1.0, 1.0, 0.0)),
    });
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
    pub throttle: f32,
    pub braking: bool,
    pub drifting: bool,
}

#[derive(Component)]
pub struct Player;

#[derive(Component)]
pub struct WheelFrontLeft;

#[derive(Component)]
pub struct WheelFrontRight;

/// Per-car tire state: tracks where the last skid decal was laid so we can
/// space them out by distance instead of spawning one every frame.
#[derive(Component)]
pub struct TireMarks {
    pub last_mark: Vec3,
}

/// A skid-mark decal on the ground; fades out (despawns) after its lifetime.
#[derive(Component)]
struct SkidMark {
    life: Timer,
}

/// Shared meshes/materials for tire effects, built once per race.
#[derive(Resource)]
struct TireAssets {
    skid_mesh: Handle<Mesh>,
    skid_mat: Handle<StandardMaterial>,
    smoke_mesh: Handle<Mesh>,
    smoke_mat: Handle<StandardMaterial>,
}

fn init_tire_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(TireAssets {
        // Thin flat quad lying on the ground = a tire mark.
        skid_mesh: meshes.add(Cuboid::new(0.4, 0.02, 0.9)),
        skid_mat: materials.add(StandardMaterial {
            base_color: Color::srgba(0.03, 0.03, 0.03, 0.85),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
        smoke_mesh: meshes.add(Sphere::new(0.25).mesh().ico(1).unwrap()),
        smoke_mat: materials.add(StandardMaterial {
            base_color: Color::srgba(0.85, 0.85, 0.85, 0.45),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
    });
}

/// Builds the visual body of a car — wheels (front two tagged for steering),
/// cabin, tinted windows, rear spoiler, head/tail lights and exhaust — as
/// children of `parent`. Shared by the player and AI so both look like cars.
pub fn build_car_visuals(
    parent: &mut ChildSpawnerCommands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    body_color: Color,
) {
    let wheel_mesh = meshes.add(Cylinder::new(0.4, 0.2));
    let wheel_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.06, 0.06, 0.06),
        perceptual_roughness: 0.9,
        ..default()
    });
    let body_mat = materials.add(body_color);
    let glass_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.05, 0.07, 0.11),
        metallic: 0.4,
        perceptual_roughness: 0.15,
        ..default()
    });
    let trim_mat = materials.add(Color::srgb(0.05, 0.05, 0.05));

    // Wheels — (x, z, is_front, is_left). Chassis forward is -Z.
    let wheels = [
        (-1.2_f32, -1.5_f32, true, true),
        (1.2, -1.5, true, false),
        (-1.2, 1.5, false, false),
        (1.2, 1.5, false, false),
    ];
    for (x, z, front, left) in wheels {
        let mut w = parent.spawn((
            Mesh3d(wheel_mesh.clone()),
            MeshMaterial3d(wheel_mat.clone()),
            Transform::from_xyz(x, -0.1, z)
                .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
        ));
        if front && left {
            w.insert(WheelFrontLeft);
        }
        if front && !left {
            w.insert(WheelFrontRight);
        }
    }

    // Cabin / roof (body colour).
    parent.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.5, 0.55, 1.7))),
        MeshMaterial3d(body_mat.clone()),
        Transform::from_xyz(0.0, 0.62, 0.2),
    ));
    // Greenhouse / windows — a dark band standing slightly proud of the cabin.
    parent.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.54, 0.4, 1.35))),
        MeshMaterial3d(glass_mat),
        Transform::from_xyz(0.0, 0.66, 0.2),
    ));
    // Rear spoiler wing.
    parent.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.8, 0.08, 0.4))),
        MeshMaterial3d(trim_mat.clone()),
        Transform::from_xyz(0.0, 0.72, 1.95),
    ));

    // Head- and tail-lights (emissive so they glow).
    let head_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 1.0, 0.9),
        emissive: Color::srgb(1.6, 1.6, 1.2).to_linear(),
        ..default()
    });
    let tail_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.4, 0.0, 0.0),
        emissive: Color::srgb(1.4, 0.0, 0.0).to_linear(),
        ..default()
    });
    for x in [-0.6_f32, 0.6] {
        parent.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.35, 0.2, 0.08))),
            MeshMaterial3d(head_mat.clone()),
            Transform::from_xyz(x, 0.1, -2.02),
        ));
        parent.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.35, 0.2, 0.08))),
            MeshMaterial3d(tail_mat.clone()),
            Transform::from_xyz(x, 0.15, 2.02),
        ));
    }

    // Exhaust port (also the smoke emitter).
    parent.spawn((
        Mesh3d(meshes.add(Cylinder::new(0.1, 0.4))),
        MeshMaterial3d(trim_mat),
        Transform::from_xyz(0.6, -0.2, 2.0)
            .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
        ExhaustPort,
    ));
}

/// Lays skid-mark decals and puffs tire smoke when a car is sliding, launching,
/// or braking hard. Works for the player and AI alike (reads throttle/brake/drift
/// off the shared `Vehicle` component).
fn tire_effects(
    mut commands: Commands,
    assets: Res<TireAssets>,
    mut query: Query<(&Vehicle, &Transform, &Velocity, &mut TireMarks)>,
) {
    for (vehicle, transform, velocity, mut marks) in query.iter_mut() {
        let forward: Vec3 = transform.forward().into();
        let right: Vec3 = transform.right().into();
        let fwd_vel = velocity.linear.dot(forward);
        let lat_vel = velocity.linear.dot(right);

        let sliding = lat_vel.abs() > 6.0;
        let launching =
            vehicle.throttle > 0.5 && fwd_vel.abs() < 14.0 && velocity.linear.length() > 1.5;
        let hard_brake = vehicle.braking && fwd_vel.abs() > 6.0;
        if !(vehicle.drifting || sliding || launching || hard_brake) {
            continue;
        }

        let yaw = forward.x.atan2(forward.z);

        // Lay marks under the rear wheels, spaced by distance travelled.
        if transform.translation.distance(marks.last_mark) > 0.8 {
            marks.last_mark = transform.translation;
            for sx in [-1.2_f32, 1.2] {
                let wp = transform.transform_point(Vec3::new(sx, -0.5, 1.5));
                let gy = crate::level_gen::get_terrain_height(wp.x, wp.z) + 0.05;
                commands.spawn((
                    Mesh3d(assets.skid_mesh.clone()),
                    MeshMaterial3d(assets.skid_mat.clone()),
                    Transform::from_xyz(wp.x, gy, wp.z).with_rotation(Quat::from_rotation_y(yaw)),
                    SkidMark {
                        life: Timer::from_seconds(12.0, TimerMode::Once),
                    },
                    RaceEntity,
                ));
            }
        }

        // Occasional smoke puff off the rear tires.
        if rand::random::<f32>() < 0.35 {
            let mut rng = rand::rng();
            let sx = if rand::random::<bool>() { -1.2 } else { 1.2 };
            let wp = transform.transform_point(Vec3::new(sx, -0.3, 1.5));
            let vel = Vec3::Y * rng.random_range(1.0..2.5)
                + right * sx.signum() * rng.random_range(0.5..1.5)
                + forward * -rng.random_range(0.0..1.5);
            commands.spawn((
                Mesh3d(assets.smoke_mesh.clone()),
                MeshMaterial3d(assets.smoke_mat.clone()),
                Transform::from_translation(wp),
                SmokeParticle {
                    timer: Timer::from_seconds(rng.random_range(0.4..0.9), TimerMode::Once),
                    velocity: vel,
                },
                RaceEntity,
            ));
        }
    }
}

fn update_skid_marks(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut SkidMark)>,
) {
    for (entity, mut mark) in query.iter_mut() {
        if mark.life.tick(time.delta()).is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

// --- Analytic terrain suspension -------------------------------------------
// The terrain has no physics collider (see level_gen); cars float on this smooth
// analytic suspension so slopes are stutter free.
const RIDE_HEIGHT: f32 = 0.6; // chassis-center height above the surface
const GROUND_FOLLOW: f32 = 8.0; // how firmly it corrects ride-height error (1/s)
const AIR_MARGIN: f32 = 1.2; // above this much clearance the car is "airborne"
const ALIGN_TORQUE: f32 = 400.0; // grounded slope-alignment spring
const AIR_ALIGN_TORQUE: f32 = 200.0; // gentler upright spring while airborne

// --- Realistic steering ----------------------------------------------------
/// Angular damping on the car body — kept in sync with the `Damping` component so
/// we can cancel it when driving the yaw rate directly (see `steering_yaw_rate`).
pub const CAR_ANGULAR_DAMPING: f32 = 20.0;
const WHEELBASE: f32 = 3.0; // front-to-rear axle distance
const MAX_LAT_ACCEL: f32 = 36.0; // grip-limited lateral acceleration (the understeer cap)

/// Target yaw rate (rad/s about world up) for a car, from a speed-sensitive
/// bicycle model. Turning is realistic and self-limiting at speed:
///
/// * Bicycle model — `yaw = v · tan(δ) / wheelbase`, so turn rate scales with speed.
/// * Grip cap — lateral accel `v · yaw` is capped, giving `yaw ≤ MAX_LAT_ACCEL / v`;
///   the car understeers and can't spin out when going fast.
/// * Speed-sensitive steering — usable lock also shrinks with speed on top of that.
///
/// `fwd_speed` is signed: negative (reversing) flips the turn direction.
pub fn steering_yaw_rate(
    fwd_speed: f32,
    steering_angle: f32,
    max_speed: f32,
    drifting: bool,
) -> f32 {
    let speed = fwd_speed.abs();
    let speed_ratio = (speed / max_speed.max(1.0)).clamp(0.0, 1.0);
    let authority = 1.0 - 0.55 * speed_ratio; // full lock when slow → 45% at top speed
    let angle = steering_angle * authority;
    let yaw = fwd_speed * angle.tan() / WHEELBASE;
    let cap = if speed > 1.0 {
        MAX_LAT_ACCEL * if drifting { 1.7 } else { 1.0 } / speed
    } else {
        f32::MAX
    };
    yaw.clamp(-cap, cap)
}

/// Keeps a car smoothly planted on and aligned to the analytic terrain surface.
///
/// Vertical motion is driven directly: we feed-forward the rate the ground rises
/// under the car (`∇h · velocity`) so it tracks the surface exactly at any speed,
/// plus a proportional term that removes residual ride-height error. Orientation
/// gets a critically-damped torque toward a smooth, wide-baseline surface normal.
/// Call this *after* the caller has set `force.force`/`force.torque` for driving.
pub fn apply_terrain_follow(
    transform: &Transform,
    velocity: &mut Velocity,
    force: &mut ExternalForce,
) {
    let pos = transform.translation;
    let ground_y = crate::level_gen::get_terrain_height(pos.x, pos.z);
    let y_error = (ground_y + RIDE_HEIGHT) - pos.y;
    let up: Vec3 = transform.up().into();

    if y_error > -AIR_MARGIN {
        // Smooth surface gradient — central difference over a wide baseline so the
        // car follows the visible slope, not fine local texture.
        const E: f32 = 4.0;
        let dhdx = (crate::level_gen::get_terrain_height(pos.x + E, pos.z)
            - crate::level_gen::get_terrain_height(pos.x - E, pos.z))
            / (2.0 * E);
        let dhdz = (crate::level_gen::get_terrain_height(pos.x, pos.z + E)
            - crate::level_gen::get_terrain_height(pos.x, pos.z - E))
            / (2.0 * E);

        // Feed-forward how fast the ground rises under us, then correct any drift.
        let surface_vy = dhdx * velocity.linear.x + dhdz * velocity.linear.z;
        velocity.linear.y = surface_vy + y_error * GROUND_FOLLOW;

        // Align the chassis "up" to the surface normal.
        let normal = Vec3::new(-dhdx, 1.0, -dhdz).normalize();
        let axis = up.cross(normal).normalize_or_zero();
        let angle = up.dot(normal).clamp(-1.0, 1.0).acos();
        force.torque += axis * angle * ALIGN_TORQUE;
    } else {
        // Airborne: let gravity bring us down, but keep roughly upright to land on wheels.
        let axis = up.cross(Vec3::Y).normalize_or_zero();
        let angle = up.dot(Vec3::Y).clamp(-1.0, 1.0).acos();
        force.torque += axis * angle * AIR_ALIGN_TORQUE;
    }
}

fn spawn_player_car(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    level_data: Res<LevelData>,
    difficulty: Res<GameDifficulty>,
) {
    let start_pos = level_data.start_pos;

    // Car chassis
    commands
        .spawn((
            Mesh3d(meshes.add(Cuboid::new(2.0, 1.0, 4.0))),
            MeshMaterial3d(materials.add(Color::srgb(0.9, 0.1, 0.1))),
            Transform::from_translation(start_pos),
            RigidBody::Dynamic,
            Collider::round_cuboid(0.9, 0.4, 1.9, 0.1),
            Velocity::default(),
            ExternalForce::default(),
            ExternalImpulse::default(),
            ReadMassProperties::default(),
            Ccd::enabled(),
            Damping {
                linear_damping: 0.5,
                angular_damping: CAR_ANGULAR_DAMPING,
            },
            Vehicle {
                speed: 0.0,
                max_speed: difficulty.top_speed,
                acceleration: difficulty.acceleration,
                steering_angle: 0.0,
                max_steering: 1.047, // 60 degrees in radians
                is_player: true,
                throttle: 0.0,
                braking: false,
                drifting: false,
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
            (
                RaceEntity,
                TireMarks {
                    last_mark: start_pos,
                },
            ),
        ))
        .with_children(|parent| {
            build_car_visuals(
                parent,
                &mut meshes,
                &mut materials,
                Color::srgb(0.9, 0.1, 0.1),
            );
        });
}

fn vehicle_update(
    time: Res<Time>,
    difficulty: Res<GameDifficulty>,
    mut query: Query<(
        &mut Vehicle,
        &mut ExternalForce,
        &Transform,
        &mut Velocity,
        Option<&Children>,
        Option<&mut crate::game_state::LapTracker>,
    )>,
    mut wheel_query: Query<
        (
            &mut Transform,
            Option<&WheelFrontLeft>,
            Option<&WheelFrontRight>,
        ),
        Without<Vehicle>,
    >,
    level_data: Res<crate::level_gen::LevelData>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    let dt = time.delta_secs();
    for (mut vehicle, mut force, transform, mut velocity, children, lap_tracker) in query.iter_mut()
    {
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

            vehicle.throttle = throttle;
            vehicle.braking = braking;
            vehicle.drifting = drifting;

            let steering_speed = difficulty.steering_sensitivity; // How fast the wheel turns
            let return_speed = difficulty.steering_sensitivity * 1.5; // How fast it returns to center

            let step = if target_steering == 0.0 {
                return_speed * dt
            } else {
                steering_speed * dt
            };
            let target_angle = target_steering * vehicle.max_steering;
            let diff = target_angle - vehicle.steering_angle;

            if diff.abs() <= step {
                vehicle.steering_angle = target_angle;
            } else {
                vehicle.steering_angle += diff.signum() * step;
            }

            // Visual wheel steering
            if let Some(children) = children {
                for child in children.iter() {
                    let child_entity = child;
                    if let Ok((mut w_transform, fl, fr)) = wheel_query.get_mut(child_entity) {
                        if fl.is_some() || fr.is_some() {
                            w_transform.rotation = Quat::from_rotation_y(vehicle.steering_angle)
                                * Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
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
            let mut grip_factor = 39.0;
            if drifting {
                grip_factor = 10.0; // Lose grip, slide!
            }
            let grip_force = -right * current_lat_vel * grip_factor;

            force.force = engine_force + drag_force + grip_force;
            force.torque = Vec3::ZERO;

            // Realistic, speed-limited turning: drive the yaw rate directly, cancelling
            // the body's angular damping so it lands on target this step.
            let target_yaw = steering_yaw_rate(
                current_fwd_vel,
                vehicle.steering_angle,
                vehicle.max_speed,
                drifting,
            );
            velocity.angular.y = target_yaw * (1.0 + CAR_ANGULAR_DAMPING * dt);

            // Smoothly follow and align to the analytic terrain (no ground collider).
            apply_terrain_follow(transform, &mut velocity, &mut force);

            // Lap tracking logic
            if let Some(mut tracker) = lap_tracker {
                if !level_data.waypoints.is_empty() {
                    let target_wp = level_data.waypoints[tracker.next_waypoint];
                    let dist = transform.translation.distance(target_wp);

                    if dist < 40.0 {
                        // Change color to yellow when approaching
                        // (Gate logic will be in a separate system or we query children here)
                    }

                    if dist < 15.0 {
                        tracker.next_waypoint += 1;
                        if tracker.next_waypoint >= level_data.waypoints.len() {
                            tracker.next_waypoint = 0;
                            tracker.current_lap += 1;
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

fn update_gate_colors(
    player_query: Query<(&Transform, &crate::game_state::LapTracker), With<Player>>,
    gate_query: Query<(&crate::game_state::WaypointMarker, &Children)>,
    gate_mats: Res<GateMaterials>,
    mut mesh_materials: Query<&mut MeshMaterial3d<StandardMaterial>>,
    level_data: Res<crate::level_gen::LevelData>,
) {
    let Some((player_transform, tracker)) = player_query.iter().next() else {
        return;
    };
    if level_data.waypoints.is_empty() {
        return;
    }

    let next_wp = tracker.next_waypoint;

    for (marker, children) in gate_query.iter() {
        let idx = marker.0;

        // Colour rules for a circular circuit:
        //   < next_wp  → green  (already passed this lap cycle)
        //   == next_wp → yellow if approaching (<= 40 m), orange otherwise
        //   > next_wp  → red    (not yet reached)
        // When next_wp wraps back to 0/1 at lap start, all higher-index gates
        // automatically revert to red without any special-case logic.
        let mat_handle = if idx == next_wp {
            let dist = player_transform
                .translation
                .distance(level_data.waypoints[idx]);
            if dist <= 40.0 {
                &gate_mats.yellow
            } else {
                &gate_mats.orange
            }
        } else if idx < next_wp {
            &gate_mats.green
        } else {
            &gate_mats.red
        };

        for child in children.iter() {
            if let Ok(mut m) = mesh_materials.get_mut(child) {
                m.0 = mat_handle.clone();
            }
        }
    }
}
