use bevy::prelude::*;
use bevy::mesh::Indices;
use rand::RngExt;
use crate::game_state::{GameState, RaceEntity};
use crate::level_gen::get_terrain_height;

pub struct PropsPlugin;

impl Plugin for PropsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (cycle_traffic_lights, drive_npc_cars).run_if(in_state(GameState::Racing)),
        );
    }
}

/// A traffic light that cycles green → yellow → red on a fixed period. Holds its
/// three lamp materials so the cycling system can toggle their glow.
#[derive(Component)]
struct TrafficLight {
    offset: f32,
    red: Handle<StandardMaterial>,
    yellow: Handle<StandardMaterial>,
    green: Handle<StandardMaterial>,
}

/// An ambient (non-racing) traffic car that cruises in a straight line and wraps
/// around the map edges.
#[derive(Component)]
struct NpcCar {
    velocity: Vec3,
}

const MAP_BOUND: f32 = 1500.0;

/// Spawns all decorative scenery + ambient traffic for a freshly generated level.
/// Everything is tagged `RaceEntity` so it's cleaned up when the race ends.
pub fn populate_world(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    waypoints: &[Vec3],
) {
    if waypoints.is_empty() {
        return;
    }
    let mut rng = rand::rng();

    spawn_trees(commands, meshes, materials, waypoints, &mut rng);
    spawn_crosswalks_and_signals(commands, meshes, materials, waypoints, &mut rng);
    let side_roads = spawn_side_roads(commands, meshes, materials, &mut rng);
    spawn_npc_cars(commands, meshes, materials, &side_roads, &mut rng);
    spawn_river_and_bridges(commands, meshes, materials, waypoints, &mut rng);
    spawn_tunnels(commands, meshes, materials, waypoints);
}

// --- Trees -----------------------------------------------------------------

fn spawn_trees(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    waypoints: &[Vec3],
    rng: &mut impl RngExt,
) {
    let trunk_mesh = meshes.add(Cylinder::new(0.35, 5.0));
    let foliage_mesh = meshes.add(Cone { radius: 2.6, height: 6.5 });
    let trunk_mat = materials.add(Color::srgb(0.32, 0.2, 0.1));
    let foliage_mats = [
        materials.add(Color::srgb(0.12, 0.42, 0.12)),
        materials.add(Color::srgb(0.16, 0.5, 0.14)),
        materials.add(Color::srgb(0.1, 0.36, 0.16)),
    ];

    let num_wp = waypoints.len();
    // Walk the loop and drop trees on the grass band just outside the road edge
    // (road half-width is 12; buildings start ~32 out, so 16..30 is clear grass).
    for i in 0..num_wp {
        let a = waypoints[i];
        let b = waypoints[(i + 1) % num_wp];
        let seg = b - a;
        let len = seg.length().max(0.001);
        let dir = (seg / len).with_y(0.0).normalize_or_zero();
        let right = Vec3::Y.cross(dir).normalize_or_zero();

        let mut d = 0.0;
        while d < len {
            d += rng.random_range(14.0..26.0);
            for side in [-1.0_f32, 1.0] {
                if rng.random_range(0.0..1.0) > 0.6 {
                    continue; // sparse, natural spacing
                }
                let off = rng.random_range(16.0..30.0);
                let base = a + dir * d + right * (off * side);
                let y = get_terrain_height(base.x, base.z);
                let scale = rng.random_range(0.7..1.5);
                commands.spawn((
                    Mesh3d(trunk_mesh.clone()),
                    MeshMaterial3d(trunk_mat.clone()),
                    Transform::from_xyz(base.x, y + 2.5 * scale, base.z)
                        .with_scale(Vec3::splat(scale)),
                    RaceEntity,
                ));
                let fmat = foliage_mats[rng.random_range(0..foliage_mats.len())].clone();
                commands.spawn((
                    Mesh3d(foliage_mesh.clone()),
                    MeshMaterial3d(fmat),
                    Transform::from_xyz(base.x, y + (5.0 + 3.0) * scale, base.z)
                        .with_scale(Vec3::splat(scale)),
                    RaceEntity,
                ));
            }
        }
    }
}

// --- Crosswalks + traffic signals ------------------------------------------

fn spawn_crosswalks_and_signals(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    waypoints: &[Vec3],
    rng: &mut impl RngExt,
) {
    let num_wp = waypoints.len();
    let stripe_mesh = meshes.add(Cuboid::new(0.5, 0.06, 22.0));
    let stripe_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        unlit: true,
        ..default()
    });
    let pole_mesh = meshes.add(Cylinder::new(0.25, 6.0));
    let housing_mesh = meshes.add(Cuboid::new(0.7, 2.0, 0.5));
    let lamp_mesh = meshes.add(Sphere::new(0.28));
    let pole_mat = materials.add(Color::srgb(0.1, 0.1, 0.12));

    // Place a crosswalk + signal every 6th gate.
    for i in (0..num_wp).step_by(6) {
        let wp = waypoints[i];
        let next = waypoints[(i + 1) % num_wp];
        let dir = (next - wp).with_y(0.0).normalize_or_zero();
        let right = Vec3::Y.cross(dir).normalize_or_zero();
        let yaw = dir.x.atan2(dir.z);

        // Zebra crossing: a row of white bars across the road.
        for k in 0..6 {
            let along = (k as f32 - 2.5) * 1.1;
            let p = wp + dir * along;
            commands.spawn((
                Mesh3d(stripe_mesh.clone()),
                MeshMaterial3d(stripe_mat.clone()),
                Transform::from_xyz(p.x, get_terrain_height(p.x, p.z) + 0.32, p.z)
                    .with_rotation(Quat::from_rotation_y(yaw)),
                RaceEntity,
            ));
        }

        // Traffic light on the right shoulder.
        let base = wp + right * 15.0;
        let by = get_terrain_height(base.x, base.z);
        let red = materials.add(lamp_material(1.0, 0.1, 0.1, false));
        let yellow = materials.add(lamp_material(1.0, 0.9, 0.1, false));
        let green = materials.add(lamp_material(0.1, 1.0, 0.2, true)); // start on green

        commands
            .spawn((
                Mesh3d(pole_mesh.clone()),
                MeshMaterial3d(pole_mat.clone()),
                Transform::from_xyz(base.x, by + 3.0, base.z),
                RaceEntity,
                TrafficLight {
                    offset: rng.random_range(0.0..8.0),
                    red: red.clone(),
                    yellow: yellow.clone(),
                    green: green.clone(),
                },
            ))
            .with_children(|p| {
                p.spawn((
                    Mesh3d(housing_mesh.clone()),
                    MeshMaterial3d(pole_mat.clone()),
                    Transform::from_xyz(0.0, 4.0, 0.0),
                ));
                p.spawn((Mesh3d(lamp_mesh.clone()), MeshMaterial3d(red), Transform::from_xyz(0.0, 4.6, 0.28)));
                p.spawn((Mesh3d(lamp_mesh.clone()), MeshMaterial3d(yellow), Transform::from_xyz(0.0, 4.0, 0.28)));
                p.spawn((Mesh3d(lamp_mesh.clone()), MeshMaterial3d(green), Transform::from_xyz(0.0, 3.4, 0.28)));
            });
    }
}

fn lamp_material(r: f32, g: f32, b: f32, on: bool) -> StandardMaterial {
    StandardMaterial {
        base_color: Color::srgb(r * 0.4, g * 0.4, b * 0.4),
        emissive: if on {
            Color::srgb(r * 3.0, g * 3.0, b * 3.0).to_linear()
        } else {
            Color::srgb(r * 0.1, g * 0.1, b * 0.1).to_linear()
        },
        ..default()
    }
}

fn cycle_traffic_lights(
    time: Res<Time>,
    lights: Query<&TrafficLight>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let t = time.elapsed_secs();
    for light in lights.iter() {
        let phase = (t + light.offset).rem_euclid(9.0);
        let (r, y, g) = if phase < 4.5 {
            (false, false, true)
        } else if phase < 5.5 {
            (false, true, false)
        } else {
            (true, false, false)
        };
        set_lamp(&mut materials, &light.red, 1.0, 0.1, 0.1, r);
        set_lamp(&mut materials, &light.yellow, 1.0, 0.9, 0.1, y);
        set_lamp(&mut materials, &light.green, 0.1, 1.0, 0.2, g);
    }
}

fn set_lamp(
    materials: &mut Assets<StandardMaterial>,
    handle: &Handle<StandardMaterial>,
    r: f32,
    g: f32,
    b: f32,
    on: bool,
) {
    if let Some(mat) = materials.get_mut(handle) {
        mat.emissive = if on {
            Color::srgb(r * 3.0, g * 3.0, b * 3.0).to_linear()
        } else {
            Color::srgb(r * 0.1, g * 0.1, b * 0.1).to_linear()
        };
    }
}

// --- Side roads + NPC traffic ----------------------------------------------

/// Returns each side road as (a start point, direction) for NPC spawning.
fn spawn_side_roads(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    rng: &mut impl RngExt,
) -> Vec<(Vec3, Vec3)> {
    let road_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.12, 0.12, 0.14),
        perceptual_roughness: 0.95,
        cull_mode: None,
        ..default()
    });

    let mut roads = Vec::new();
    for _ in 0..4 {
        // A straight avenue crossing the whole map at a random offset/heading.
        let angle = rng.random_range(0.0..std::f32::consts::TAU);
        let dir = Vec3::new(angle.cos(), 0.0, angle.sin());
        let perp = Vec3::Y.cross(dir).normalize_or_zero();
        let offset = perp * rng.random_range(-900.0..900.0);

        let mut centerline = Vec::new();
        let n = 60;
        for k in 0..=n {
            let t = k as f32 / n as f32;
            let p = offset + dir * ((t - 0.5) * 2.0 * MAP_BOUND);
            centerline.push(Vec3::new(p.x, 0.0, p.z));
        }

        commands.spawn((
            Mesh3d(meshes.add(draped_strip_mesh(&centerline, 7.0, [0.12, 0.12, 0.14, 1.0], 0.28, None))),
            MeshMaterial3d(road_mat.clone()),
            Transform::IDENTITY,
            RaceEntity,
        ));

        roads.push((offset - dir * MAP_BOUND, dir));
    }
    roads
}

fn spawn_npc_cars(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    side_roads: &[(Vec3, Vec3)],
    rng: &mut impl RngExt,
) {
    if side_roads.is_empty() {
        return;
    }
    let body_mesh = meshes.add(Cuboid::new(2.0, 1.0, 4.2));
    let cabin_mesh = meshes.add(Cuboid::new(1.6, 0.6, 1.8));
    let cabin_mat = materials.add(Color::srgb(0.08, 0.09, 0.12));
    let colors = [
        Color::srgb(0.85, 0.85, 0.9),
        Color::srgb(0.2, 0.3, 0.7),
        Color::srgb(0.7, 0.6, 0.2),
        Color::srgb(0.6, 0.15, 0.15),
        Color::srgb(0.15, 0.15, 0.18),
    ];

    for _ in 0..14 {
        let (start, dir) = side_roads[rng.random_range(0..side_roads.len())];
        // Random point along the road, in one of the two lanes.
        let along = rng.random_range(0.0..(2.0 * MAP_BOUND));
        let perp = Vec3::Y.cross(dir).normalize_or_zero();
        let lane = if rng.random_range(0.0..1.0) < 0.5 { -3.5 } else { 3.5 };
        let travel = if lane < 0.0 { dir } else { -dir };
        let p = start + dir * along + perp * lane;
        let y = get_terrain_height(p.x, p.z) + 0.6;
        let speed = rng.random_range(12.0..26.0);
        let color = colors[rng.random_range(0..colors.len())];

        commands
            .spawn((
                Mesh3d(body_mesh.clone()),
                MeshMaterial3d(materials.add(color)),
                Transform::from_xyz(p.x, y, p.z),
                NpcCar {
                    velocity: travel * speed,
                },
                RaceEntity,
            ))
            .with_children(|c| {
                c.spawn((
                    Mesh3d(cabin_mesh.clone()),
                    MeshMaterial3d(cabin_mat.clone()),
                    Transform::from_xyz(0.0, 0.6, 0.1),
                ));
            });
    }
}

fn drive_npc_cars(time: Res<Time>, mut query: Query<(&mut Transform, &NpcCar)>) {
    let dt = time.delta_secs();
    for (mut tf, npc) in query.iter_mut() {
        tf.translation += npc.velocity * dt;

        // Wrap around the map so traffic keeps flowing.
        if tf.translation.x > MAP_BOUND {
            tf.translation.x = -MAP_BOUND;
        } else if tf.translation.x < -MAP_BOUND {
            tf.translation.x = MAP_BOUND;
        }
        if tf.translation.z > MAP_BOUND {
            tf.translation.z = -MAP_BOUND;
        } else if tf.translation.z < -MAP_BOUND {
            tf.translation.z = MAP_BOUND;
        }

        tf.translation.y = get_terrain_height(tf.translation.x, tf.translation.z) + 0.6;
        if npc.velocity.length_squared() > 0.01 {
            let dir = npc.velocity.normalize();
            tf.look_to(dir, Vec3::Y); // car forward (-Z) faces travel direction
        }
    }
}

// --- Rivers + bridges + tunnels (decorative terrain variety) ---------------

fn spawn_river_and_bridges(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    waypoints: &[Vec3],
    rng: &mut impl RngExt,
) {
    // A gently winding river crossing the map. It's a flat translucent strip set
    // near the valley floor, so it shows through low ground.
    let angle = rng.random_range(0.0..std::f32::consts::TAU);
    let dir = Vec3::new(angle.cos(), 0.0, angle.sin());
    let perp = Vec3::Y.cross(dir).normalize_or_zero();
    let wobble_amp = rng.random_range(120.0..260.0);
    let wobble_ph = rng.random_range(0.0..std::f32::consts::TAU);

    let mut centerline = Vec::new();
    let n = 80;
    let mut min_terrain = f32::MAX;
    for k in 0..=n {
        let t = k as f32 / n as f32;
        let along = (t - 0.5) * 2.0 * MAP_BOUND;
        let wob = (t * 6.0 + wobble_ph).sin() * wobble_amp;
        let p = dir * along + perp * wob;
        min_terrain = min_terrain.min(get_terrain_height(p.x, p.z));
        centerline.push(Vec3::new(p.x, 0.0, p.z));
    }
    let water_y = min_terrain + 3.0;

    let water_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.1, 0.35, 0.6, 0.7),
        perceptual_roughness: 0.1,
        metallic: 0.0,
        cull_mode: None,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(draped_strip_mesh(&centerline, 22.0, [0.1, 0.35, 0.6, 0.7], 0.0, Some(water_y)))),
        MeshMaterial3d(water_mat),
        Transform::IDENTITY,
        RaceEntity,
    ));

    // Where the racing line passes closest to the river, dress that stretch as a
    // bridge: side railings and support pillars dropping toward the water.
    let num_wp = waypoints.len();
    let mut best = (f32::MAX, 0usize);
    for i in 0..num_wp {
        let wp = waypoints[i];
        // distance from wp to the river polyline (coarse)
        let mut dmin = f32::MAX;
        for c in centerline.iter() {
            dmin = dmin.min(Vec3::new(wp.x, 0.0, wp.z).distance(*c));
        }
        if dmin < best.0 {
            best = (dmin, i);
        }
    }

    let rail_mat = materials.add(Color::srgb(0.5, 0.5, 0.55));
    let pillar_mat = materials.add(Color::srgb(0.4, 0.4, 0.42));
    let bridge_i = best.1;
    for j in -2i32..=2 {
        let idx = ((bridge_i as i32 + j).rem_euclid(num_wp as i32)) as usize;
        let wp = waypoints[idx];
        let next = waypoints[(idx + 1) % num_wp];
        let dir = (next - wp).with_y(0.0).normalize_or_zero();
        let right = Vec3::Y.cross(dir).normalize_or_zero();
        let road_y = get_terrain_height(wp.x, wp.z) + 0.4;
        let yaw = dir.x.atan2(dir.z);

        // Railings on both sides of the road.
        for side in [-1.0_f32, 1.0] {
            let p = wp + right * (12.5 * side);
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.4, 1.2, 30.0))),
                MeshMaterial3d(rail_mat.clone()),
                Transform::from_xyz(p.x, road_y + 0.6, p.z).with_rotation(Quat::from_rotation_y(yaw)),
                RaceEntity,
            ));
            // A support pillar dropping to the water.
            let pillar_h = (road_y - water_y).max(2.0);
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(1.5, pillar_h, 1.5))),
                MeshMaterial3d(pillar_mat.clone()),
                Transform::from_xyz(p.x, water_y + pillar_h / 2.0, p.z),
                RaceEntity,
            ));
        }
    }
}

fn spawn_tunnels(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    waypoints: &[Vec3],
) {
    let num_wp = waypoints.len();
    // Find the highest point on the loop and frame the road there with a portal.
    let mut hi = (f32::MIN, 0usize);
    for i in 0..num_wp {
        let h = get_terrain_height(waypoints[i].x, waypoints[i].z);
        if h > hi.0 {
            hi = (h, i);
        }
    }

    let portal_mat = materials.add(Color::srgb(0.3, 0.3, 0.34));
    let side_mesh = meshes.add(Cuboid::new(2.0, 9.0, 3.0));
    let top_mesh = meshes.add(Cuboid::new(28.0, 2.0, 3.0));

    for j in [-1i32, 1] {
        let idx = ((hi.1 as i32 + j * 2).rem_euclid(num_wp as i32)) as usize;
        let wp = waypoints[idx];
        let next = waypoints[(idx + 1) % num_wp];
        let dir = (next - wp).with_y(0.0).normalize_or_zero();
        let right = Vec3::Y.cross(dir).normalize_or_zero();
        let yaw = dir.x.atan2(dir.z);
        let gy = get_terrain_height(wp.x, wp.z);

        for side in [-1.0_f32, 1.0] {
            let p = wp + right * (13.0 * side);
            commands.spawn((
                Mesh3d(side_mesh.clone()),
                MeshMaterial3d(portal_mat.clone()),
                Transform::from_xyz(p.x, gy + 4.5, p.z).with_rotation(Quat::from_rotation_y(yaw)),
                RaceEntity,
            ));
        }
        commands.spawn((
            Mesh3d(top_mesh.clone()),
            MeshMaterial3d(portal_mat.clone()),
            Transform::from_xyz(wp.x, gy + 9.0, wp.z).with_rotation(Quat::from_rotation_y(yaw)),
            RaceEntity,
        ));
    }
}

// --- Mesh helper -----------------------------------------------------------

/// Builds a flat strip of quads down a centreline. If `flat_y` is given the whole
/// strip sits at that height (water); otherwise each vertex is draped onto the
/// terrain at `y_off` above it (roads).
fn draped_strip_mesh(
    centerline: &[Vec3],
    half_width: f32,
    color: [f32; 4],
    y_off: f32,
    flat_y: Option<f32>,
) -> Mesh {
    let n = centerline.len();
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    for i in 0..n {
        let c = centerline[i];
        let prev = centerline[i.saturating_sub(1)];
        let next = centerline[(i + 1).min(n - 1)];
        let dir = Vec3::new(next.x - prev.x, 0.0, next.z - prev.z).normalize_or_zero();
        let right = Vec3::Y.cross(dir).normalize_or_zero();
        for side in [-1.0_f32, 1.0] {
            let p = c + right * (half_width * side);
            let y = match flat_y {
                Some(fy) => fy,
                None => get_terrain_height(p.x, p.z) + y_off,
            };
            positions.push([p.x, y, p.z]);
            normals.push([0.0, 1.0, 0.0]);
            uvs.push([0.0, 0.0]);
            colors.push(color);
        }
    }

    for i in 0..n.saturating_sub(1) {
        let a = (i * 2) as u32;
        indices.extend_from_slice(&[a, a + 2, a + 1, a + 1, a + 2, a + 3]);
    }

    let mut mesh = Mesh::new(
        bevy::render::render_resource::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}
