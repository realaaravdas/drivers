use bevy::prelude::*;
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

// City street grid (matches the streets baked into the terrain in level_gen):
// streets run on the 40 m block grid, offset 20 m so building blocks sit between.
const BLOCK: f32 = 40.0;
const STREET_OFFSET: f32 = 20.0;
const CITY_HALF: f32 = 900.0; // scatter props within ±this
const MAP_BOUND: f32 = 1400.0;
const MAX_TREES: usize = 340;

/// A traffic light that cycles green → yellow → red. Holds its three lamp
/// materials so the cycling system can toggle their glow.
#[derive(Component)]
struct TrafficLight {
    offset: f32,
    red: Handle<StandardMaterial>,
    yellow: Handle<StandardMaterial>,
    green: Handle<StandardMaterial>,
}

/// An ambient (non-racing) traffic car that cruises along a city street.
#[derive(Component)]
struct NpcCar {
    velocity: Vec3,
}

fn grid_indices() -> std::ops::RangeInclusive<i32> {
    let n = (CITY_HALF / BLOCK) as i32;
    -n..=n
}

fn street_coord(k: i32) -> f32 {
    k as f32 * BLOCK + STREET_OFFSET
}

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
    spawn_signals(commands, meshes, materials, waypoints, &mut rng);
    spawn_npc_cars(commands, meshes, materials, &mut rng);
    spawn_water_and_bridges(commands, meshes, materials, waypoints, &mut rng);
    spawn_tunnel(commands, meshes, materials, waypoints);
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

    let ks: Vec<i32> = grid_indices().collect();

    // 1) Street trees lining the city grid. Randomly sampled across the WHOLE grid
    // (rather than filling one street at a time) so they're spread evenly.
    for _ in 0..MAX_TREES {
        let k = ks[rng.random_range(0..ks.len())];
        let along = rng.random_range(-CITY_HALF..CITY_HALF);
        let side = if rng.random_range(0.0..1.0) < 0.5 { -1.0 } else { 1.0 };
        // Half the trees line vertical streets, half line horizontal ones.
        let base = if rng.random_range(0.0..1.0) < 0.5 {
            Vec3::new(street_coord(k) + 10.0 * side, 0.0, along)
        } else {
            Vec3::new(along, 0.0, street_coord(k) + 10.0 * side)
        };
        let scale = rng.random_range(0.7..1.5);
        let fmat = &foliage_mats[rng.random_range(0..foliage_mats.len())];
        plant_tree(commands, &trunk_mesh, &foliage_mesh, &trunk_mat, fmat, base, scale);
    }

    // 2) Extra trees hugging the racing road so the circuit itself is tree-lined.
    let num_wp = waypoints.len();
    for i in 0..num_wp {
        let a = waypoints[i];
        let b = waypoints[(i + 1) % num_wp];
        let seg = b - a;
        let len = seg.length().max(0.001);
        let dir = Vec3::new(seg.x, 0.0, seg.z).normalize_or_zero();
        let right = Vec3::Y.cross(dir).normalize_or_zero();
        let mut d = 0.0;
        while d < len {
            d += 22.0;
            for side in [-1.0_f32, 1.0] {
                if rng.random_range(0.0..1.0) < 0.5 {
                    continue;
                }
                let off = rng.random_range(13.0..20.0);
                let base = a + dir * d + right * (off * side);
                let scale = rng.random_range(0.7..1.5);
                let fmat = &foliage_mats[rng.random_range(0..foliage_mats.len())];
                plant_tree(commands, &trunk_mesh, &foliage_mesh, &trunk_mat, fmat, base, scale);
            }
        }
    }
}

fn plant_tree(
    commands: &mut Commands,
    trunk_mesh: &Handle<Mesh>,
    foliage_mesh: &Handle<Mesh>,
    trunk_mat: &Handle<StandardMaterial>,
    foliage_mat: &Handle<StandardMaterial>,
    base: Vec3,
    scale: f32,
) {
    let y = get_terrain_height(base.x, base.z);
    commands.spawn((
        Mesh3d(trunk_mesh.clone()),
        MeshMaterial3d(trunk_mat.clone()),
        Transform::from_xyz(base.x, y + 2.5 * scale, base.z).with_scale(Vec3::splat(scale)),
        RaceEntity,
    ));
    commands.spawn((
        Mesh3d(foliage_mesh.clone()),
        MeshMaterial3d(foliage_mat.clone()),
        Transform::from_xyz(base.x, y + 8.0 * scale, base.z).with_scale(Vec3::splat(scale)),
        RaceEntity,
    ));
}

// --- Traffic signals -------------------------------------------------------

fn spawn_signals(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    waypoints: &[Vec3],
    rng: &mut impl RngExt,
) {
    let pole_mesh = meshes.add(Cylinder::new(0.4, 11.0));
    let arm_mesh = meshes.add(Cuboid::new(6.0, 0.4, 0.4));
    let housing_mesh = meshes.add(Cuboid::new(1.2, 3.2, 0.8));
    let lamp_mesh = meshes.add(Sphere::new(0.5));
    let pole_mat = materials.add(Color::srgb(0.08, 0.08, 0.1));

    // Big, obvious signals on the racing circuit itself (every 3rd gate), so you
    // actually see them while racing.
    let num_wp = waypoints.len();
    for i in (0..num_wp).step_by(3) {
        let wp = waypoints[i];
        let next = waypoints[(i + 1) % num_wp];
        let dir = Vec3::new(next.x - wp.x, 0.0, next.z - wp.z).normalize_or_zero();
        let right = Vec3::Y.cross(dir).normalize_or_zero();
        let base = wp + right * 13.0;
        let by = get_terrain_height(base.x, base.z);

        let red = materials.add(lamp_material(1.0, 0.1, 0.1, false));
        let yellow = materials.add(lamp_material(1.0, 0.9, 0.1, false));
        let green = materials.add(lamp_material(0.1, 1.0, 0.2, true));

        // Signal head hangs over the road on an arm.
        let over = -right * 6.0; // toward the road centre
        commands
            .spawn((
                Mesh3d(pole_mesh.clone()),
                MeshMaterial3d(pole_mat.clone()),
                Transform::from_xyz(base.x, by + 5.5, base.z),
                RaceEntity,
                TrafficLight {
                    offset: rng.random_range(0.0..9.0),
                    red: red.clone(),
                    yellow: yellow.clone(),
                    green: green.clone(),
                },
            ))
            .with_children(|p| {
                let yaw = dir.x.atan2(dir.z);
                p.spawn((
                    Mesh3d(arm_mesh.clone()),
                    MeshMaterial3d(pole_mat.clone()),
                    Transform::from_xyz(over.x * 0.5, 5.0, over.z * 0.5)
                        .with_rotation(Quat::from_rotation_y(yaw)),
                ));
                let head = Vec3::new(over.x, 4.6, over.z);
                p.spawn((
                    Mesh3d(housing_mesh.clone()),
                    MeshMaterial3d(pole_mat.clone()),
                    Transform::from_translation(head),
                ));
                p.spawn((Mesh3d(lamp_mesh.clone()), MeshMaterial3d(red), Transform::from_xyz(head.x, head.y + 1.0, head.z + 0.4)));
                p.spawn((Mesh3d(lamp_mesh.clone()), MeshMaterial3d(yellow), Transform::from_xyz(head.x, head.y, head.z + 0.4)));
                p.spawn((Mesh3d(lamp_mesh.clone()), MeshMaterial3d(green), Transform::from_xyz(head.x, head.y - 1.0, head.z + 0.4)));
            });
    }
}

fn lamp_material(r: f32, g: f32, b: f32, on: bool) -> StandardMaterial {
    StandardMaterial {
        base_color: Color::srgb(r * 0.4, g * 0.4, b * 0.4),
        emissive: lamp_emissive(r, g, b, on),
        ..default()
    }
}

fn lamp_emissive(r: f32, g: f32, b: f32, on: bool) -> LinearRgba {
    if on {
        Color::srgb(r * 6.0, g * 6.0, b * 6.0).to_linear()
    } else {
        Color::srgb(r * 0.15, g * 0.15, b * 0.15).to_linear()
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
        if let Some(m) = materials.get_mut(&light.red) {
            m.emissive = lamp_emissive(1.0, 0.1, 0.1, r);
        }
        if let Some(m) = materials.get_mut(&light.yellow) {
            m.emissive = lamp_emissive(1.0, 0.9, 0.1, y);
        }
        if let Some(m) = materials.get_mut(&light.green) {
            m.emissive = lamp_emissive(0.1, 1.0, 0.2, g);
        }
    }
}

// --- NPC traffic -----------------------------------------------------------

fn spawn_npc_cars(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    rng: &mut impl RngExt,
) {
    let body_mesh = meshes.add(Cuboid::new(2.0, 1.0, 4.2));
    let cabin_mesh = meshes.add(Cuboid::new(1.6, 0.6, 1.8));
    let cabin_mat = materials.add(Color::srgb(0.08, 0.09, 0.12));
    let colors = [
        Color::srgb(0.85, 0.85, 0.9),
        Color::srgb(0.2, 0.3, 0.7),
        Color::srgb(0.75, 0.65, 0.2),
        Color::srgb(0.6, 0.15, 0.15),
        Color::srgb(0.15, 0.15, 0.18),
        Color::srgb(0.2, 0.5, 0.3),
    ];

    // Plenty of ambient traffic, all riding the city street grid.
    for _ in 0..34 {
        let k = rng.random_range(grid_indices());
        let vertical = rng.random_range(0.0..1.0) < 0.5; // street runs along z?
        let along = rng.random_range(-CITY_HALF..CITY_HALF);
        let lane = if rng.random_range(0.0..1.0) < 0.5 { -3.0 } else { 3.0 };
        let speed = rng.random_range(12.0..26.0);
        let color = colors[rng.random_range(0..colors.len())];

        // Position on the chosen street, and velocity down its length.
        let (pos, vel) = if vertical {
            let x = street_coord(k) + lane;
            let dir = if lane < 0.0 { 1.0 } else { -1.0 };
            (Vec3::new(x, 0.0, along), Vec3::new(0.0, 0.0, dir * speed))
        } else {
            let z = street_coord(k) + lane;
            let dir = if lane < 0.0 { 1.0 } else { -1.0 };
            (Vec3::new(along, 0.0, z), Vec3::new(dir * speed, 0.0, 0.0))
        };
        let y = get_terrain_height(pos.x, pos.z) + 0.6;

        commands
            .spawn((
                Mesh3d(body_mesh.clone()),
                MeshMaterial3d(materials.add(color)),
                Transform::from_xyz(pos.x, y, pos.z),
                NpcCar { velocity: vel },
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

        // Axis-aligned velocity + independent wrap keeps each car on its street.
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
            tf.look_to(npc.velocity.normalize(), Vec3::Y);
        }
    }
}

// --- Water (lakes/rivers) + bridges ----------------------------------------

fn spawn_water_and_bridges(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    waypoints: &[Vec3],
    rng: &mut impl RngExt,
) {
    // Find the two lowest basins by sampling a coarse grid, and drop a lake into
    // each so there's visible water down in the valleys.
    let mut samples: Vec<(f32, Vec3)> = Vec::new();
    let step = 140.0;
    let mut gx = -1200.0;
    while gx <= 1200.0 {
        let mut gz = -1200.0;
        while gz <= 1200.0 {
            let h = get_terrain_height(gx, gz);
            samples.push((h, Vec3::new(gx, 0.0, gz)));
            gz += step;
        }
        gx += step;
    }
    samples.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    let water_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.1, 0.35, 0.62, 0.75),
        perceptual_roughness: 0.08,
        metallic: 0.1,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });

    let mut lake_centres: Vec<(Vec3, f32)> = Vec::new();
    for (h, centre) in samples.iter().take(2) {
        let surface = h + 1.5;
        let (w, d) = (rng.random_range(260.0..360.0), rng.random_range(180.0..260.0));
        // A slab sunk into the basin; surrounding higher terrain hides the edges,
        // leaving a natural lake shape.
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(w, 8.0, d))),
            MeshMaterial3d(water_mat.clone()),
            Transform::from_xyz(centre.x, surface - 4.0, centre.z),
            RaceEntity,
        ));
        lake_centres.push((*centre, surface));
    }

    // Bridge: dress the stretch of racing road that passes nearest a lake with
    // railings and support pillars dropping toward the water.
    if lake_centres.is_empty() {
        return;
    }
    let (lake_c, water_y) = lake_centres[0];
    let num_wp = waypoints.len();
    let mut best = (f32::MAX, 0usize);
    for i in 0..num_wp {
        let d = Vec3::new(waypoints[i].x, 0.0, waypoints[i].z).distance(lake_c);
        if d < best.0 {
            best = (d, i);
        }
    }
    let rail_mat = materials.add(Color::srgb(0.55, 0.55, 0.6));
    let pillar_mat = materials.add(Color::srgb(0.42, 0.42, 0.45));
    for j in -2i32..=2 {
        let idx = ((best.1 as i32 + j).rem_euclid(num_wp as i32)) as usize;
        let wp = waypoints[idx];
        let next = waypoints[(idx + 1) % num_wp];
        let dir = Vec3::new(next.x - wp.x, 0.0, next.z - wp.z).normalize_or_zero();
        let right = Vec3::Y.cross(dir).normalize_or_zero();
        let road_y = get_terrain_height(wp.x, wp.z) + 0.4;
        let yaw = dir.x.atan2(dir.z);

        for side in [-1.0_f32, 1.0] {
            let p = wp + right * (10.5 * side);
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.4, 1.2, 26.0))),
                MeshMaterial3d(rail_mat.clone()),
                Transform::from_xyz(p.x, road_y + 0.6, p.z).with_rotation(Quat::from_rotation_y(yaw)),
                RaceEntity,
            ));
            let pillar_h = (road_y - water_y).max(2.0);
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(1.6, pillar_h, 1.6))),
                MeshMaterial3d(pillar_mat.clone()),
                Transform::from_xyz(p.x, water_y + pillar_h / 2.0 - 4.0, p.z),
                RaceEntity,
            ));
        }
    }
}

// --- Tunnel (drivable) -----------------------------------------------------

fn spawn_tunnel(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    waypoints: &[Vec3],
) {
    let num_wp = waypoints.len();
    // Highest point on the loop → run a covered tunnel through the hill there.
    let mut hi = (f32::MIN, 0usize);
    for i in 0..num_wp {
        let h = get_terrain_height(waypoints[i].x, waypoints[i].z);
        if h > hi.0 {
            hi = (h, i);
        }
    }
    let i = hi.1;
    let a = waypoints[i];
    let b = waypoints[(i + 1) % num_wp];
    let dir = Vec3::new(b.x - a.x, 0.0, b.z - a.z).normalize_or_zero();
    let right = Vec3::Y.cross(dir).normalize_or_zero();
    let yaw = dir.x.atan2(dir.z);

    let wall_mat = materials.add(Color::srgb(0.28, 0.28, 0.32));
    let wall_mesh = meshes.add(Cuboid::new(2.0, 8.0, 6.0));
    let roof_mesh = meshes.add(Cuboid::new(24.0, 1.5, 6.0));

    // Build ~60 m of covered corridor centred on the peak.
    let mut d = -30.0_f32;
    while d <= 30.0 {
        let c = a + dir * d;
        let gy = get_terrain_height(c.x, c.z);
        for side in [-1.0_f32, 1.0] {
            let p = c + right * (11.0 * side);
            commands.spawn((
                Mesh3d(wall_mesh.clone()),
                MeshMaterial3d(wall_mat.clone()),
                Transform::from_xyz(p.x, gy + 4.0, p.z).with_rotation(Quat::from_rotation_y(yaw)),
                RaceEntity,
            ));
        }
        commands.spawn((
            Mesh3d(roof_mesh.clone()),
            MeshMaterial3d(wall_mat.clone()),
            Transform::from_xyz(c.x, gy + 8.0, c.z).with_rotation(Quat::from_rotation_y(yaw)),
            RaceEntity,
        ));
        d += 6.0;
    }
}
