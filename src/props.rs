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

#[derive(Component)]
struct TrafficLight {
    offset: f32,
    red: Handle<StandardMaterial>,
    yellow: Handle<StandardMaterial>,
    green: Handle<StandardMaterial>,
}

/// Ambient traffic car that drives back and forth along a straight avenue.
#[derive(Component)]
struct NpcCar {
    origin: Vec3,
    dir: Vec3,
    length: f32,
    speed: f32,
    t: f32,
}

fn dist_to_pts(x: f32, z: f32, pts: &[Vec3]) -> f32 {
    let mut best = f32::MAX;
    for p in pts {
        let dx = p.x - x;
        let dz = p.z - z;
        best = best.min(dx * dx + dz * dz);
    }
    best.sqrt()
}

pub fn populate_world(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    waypoints: &[Vec3],
    road_centerline: &[Vec3],
    avenues: &[Vec<Vec3>],
) {
    if waypoints.is_empty() || road_centerline.is_empty() {
        return;
    }
    let mut rng = rand::rng();

    spawn_trees(commands, meshes, materials, avenues, road_centerline, &mut rng);
    spawn_signals(commands, meshes, materials, waypoints, road_centerline, &mut rng);
    spawn_npc_cars(commands, meshes, materials, avenues, &mut rng);
    spawn_lakes_and_bridge(commands, meshes, materials, road_centerline, &mut rng);
    spawn_tunnel(commands, meshes, materials, road_centerline);
}

// --- Trees -----------------------------------------------------------------

fn spawn_trees(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    avenues: &[Vec<Vec3>],
    road_centerline: &[Vec3],
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

    // Tree-lined avenues: walk each avenue and plant on both sides, clear of the
    // racing road.
    for av in avenues.iter() {
        if av.len() < 2 {
            continue;
        }
        let dir = (av[av.len() - 1] - av[0]).normalize_or_zero();
        let right = Vec3::Y.cross(dir).normalize_or_zero();
        let mut i = 0;
        while i < av.len() {
            let c = av[i];
            i += 2; // avenue samples are 16 m apart → a tree ~every 32 m
            for side in [-1.0_f32, 1.0] {
                if rng.random_range(0.0..1.0) < 0.4 {
                    continue;
                }
                let base = c + right * (9.0 * side);
                if dist_to_pts(base.x, base.z, road_centerline) < 14.0 {
                    continue; // never on the racing road
                }
                plant_tree(commands, &trunk_mesh, &foliage_mesh, &trunk_mat,
                    &foliage_mats[rng.random_range(0..foliage_mats.len())],
                    base, rng.random_range(0.7..1.5));
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
    road_centerline: &[Vec3],
    rng: &mut impl RngExt,
) {
    let pole_mesh = meshes.add(Cylinder::new(0.4, 11.0));
    let arm_mesh = meshes.add(Cuboid::new(7.0, 0.4, 0.4));
    let housing_mesh = meshes.add(Cuboid::new(1.3, 3.4, 0.9));
    let lamp_mesh = meshes.add(Sphere::new(0.55));
    let pole_mat = materials.add(Color::srgb(0.08, 0.08, 0.1));

    let num_wp = waypoints.len();
    // A big signal on the circuit at every other gate.
    for i in (0..num_wp).step_by(2) {
        let wp = waypoints[i];
        let next = waypoints[(i + 1) % num_wp];
        let dir = Vec3::new(next.x - wp.x, 0.0, next.z - wp.z).normalize_or_zero();
        let right = Vec3::Y.cross(dir).normalize_or_zero();
        let base = wp + right * 13.0; // on the shoulder, off the 20 m road
        if dist_to_pts(base.x, base.z, road_centerline) < 10.0 {
            continue;
        }
        let by = get_terrain_height(base.x, base.z);
        let yaw = dir.x.atan2(dir.z);

        let red = materials.add(lamp_material(1.0, 0.1, 0.1, false));
        let yellow = materials.add(lamp_material(1.0, 0.9, 0.1, false));
        let green = materials.add(lamp_material(0.1, 1.0, 0.2, true));

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
                // Arm + signal head hang over the road (toward its centre).
                p.spawn((
                    Mesh3d(arm_mesh.clone()),
                    MeshMaterial3d(pole_mat.clone()),
                    Transform::from_xyz(0.0, 5.0, -3.5).with_rotation(Quat::from_rotation_y(yaw)),
                ));
                let head = Vec3::new(0.0, 4.6, -7.0);
                p.spawn((
                    Mesh3d(housing_mesh.clone()),
                    MeshMaterial3d(pole_mat.clone()),
                    Transform::from_translation(head).with_rotation(Quat::from_rotation_y(yaw)),
                ));
                p.spawn((Mesh3d(lamp_mesh.clone()), MeshMaterial3d(red), Transform::from_xyz(head.x, head.y + 1.1, head.z)));
                p.spawn((Mesh3d(lamp_mesh.clone()), MeshMaterial3d(yellow), Transform::from_xyz(head.x, head.y, head.z)));
                p.spawn((Mesh3d(lamp_mesh.clone()), MeshMaterial3d(green), Transform::from_xyz(head.x, head.y - 1.1, head.z)));
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

// --- NPC traffic on avenues ------------------------------------------------

fn spawn_npc_cars(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    avenues: &[Vec<Vec3>],
    rng: &mut impl RngExt,
) {
    if avenues.is_empty() {
        return;
    }
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

    // ~3 cars per avenue → lively traffic on the whole network.
    for av in avenues.iter() {
        if av.len() < 2 {
            continue;
        }
        let start = av[0];
        let end = av[av.len() - 1];
        let full = end - start;
        let length = full.length().max(1.0);
        let dir = full / length;
        let right = Vec3::Y.cross(dir).normalize_or_zero();

        for _ in 0..3 {
            let lane = if rng.random_range(0.0..1.0) < 0.5 { -3.0 } else { 3.0 };
            let travel = if lane < 0.0 { dir } else { -dir };
            let origin = start + right * lane;
            let t = rng.random_range(0.0..length);
            let speed = rng.random_range(12.0..26.0);
            let color = colors[rng.random_range(0..colors.len())];
            let p = origin + travel * t;
            let y = get_terrain_height(p.x, p.z) + 0.6;

            commands
                .spawn((
                    Mesh3d(body_mesh.clone()),
                    MeshMaterial3d(materials.add(color)),
                    Transform::from_xyz(p.x, y, p.z),
                    NpcCar {
                        origin,
                        dir: travel,
                        length,
                        speed,
                        t,
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
}

fn drive_npc_cars(time: Res<Time>, mut query: Query<(&mut Transform, &mut NpcCar)>) {
    let dt = time.delta_secs();
    for (mut tf, mut npc) in query.iter_mut() {
        npc.t += npc.speed * dt;
        if npc.t > npc.length {
            npc.t -= npc.length; // loop back along the avenue
        }
        let p = npc.origin + npc.dir * npc.t;
        tf.translation = Vec3::new(p.x, get_terrain_height(p.x, p.z) + 0.6, p.z);
        tf.look_to(npc.dir, Vec3::Y);
    }
}

// --- Lakes + a giant bridge on the circuit ---------------------------------

fn spawn_lakes_and_bridge(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    road_centerline: &[Vec3],
    rng: &mut impl RngExt,
) {
    // Lakes in the two lowest basins.
    let mut samples: Vec<(f32, Vec3)> = Vec::new();
    let mut gx = -1200.0;
    while gx <= 1200.0 {
        let mut gz = -1200.0;
        while gz <= 1200.0 {
            samples.push((get_terrain_height(gx, gz), Vec3::new(gx, 0.0, gz)));
            gz += 140.0;
        }
        gx += 140.0;
    }
    samples.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    let water_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.1, 0.35, 0.62, 0.75),
        perceptual_roughness: 0.08,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    for (h, c) in samples.iter().take(2) {
        let (w, d) = (rng.random_range(280.0..380.0), rng.random_range(200.0..280.0));
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(w, 8.0, d))),
            MeshMaterial3d(water_mat.clone()),
            Transform::from_xyz(c.x, h + 1.5 - 4.0, c.z),
            RaceEntity,
        ));
    }

    // Giant bridge where the racing road passes its LOWEST point (a valley): two
    // tall towers flanking the road, deck railings, support pillars and cables.
    let mut low = (f32::MAX, 0usize);
    for (i, c) in road_centerline.iter().enumerate() {
        let h = get_terrain_height(c.x, c.z);
        if h < low.0 {
            low = (h, i);
        }
    }
    let n = road_centerline.len();
    let li = low.1;
    let c = road_centerline[li];
    let ahead = road_centerline[(li + 1) % n];
    let dir = Vec3::new(ahead.x - c.x, 0.0, ahead.z - c.z).normalize_or_zero();
    let right = Vec3::Y.cross(dir).normalize_or_zero();
    let yaw = dir.x.atan2(dir.z);
    let road_y = get_terrain_height(c.x, c.z) + 0.4;

    let steel = materials.add(Color::srgb(0.75, 0.2, 0.15)); // "golden gate" red
    let cable_mat = materials.add(Color::srgb(0.3, 0.1, 0.08));
    let tower_mesh = meshes.add(Cuboid::new(3.0, 46.0, 3.0));
    let cross_mesh = meshes.add(Cuboid::new(26.0, 2.5, 2.0));

    for side in [-1.0_f32, 1.0] {
        let p = c + right * (12.0 * side);
        // Tall tower.
        commands.spawn((
            Mesh3d(tower_mesh.clone()),
            MeshMaterial3d(steel.clone()),
            Transform::from_xyz(p.x, road_y + 23.0, p.z).with_rotation(Quat::from_rotation_y(yaw)),
            RaceEntity,
        ));
    }
    // Cross-beam over the road linking the towers.
    commands.spawn((
        Mesh3d(cross_mesh.clone()),
        MeshMaterial3d(steel.clone()),
        Transform::from_xyz(c.x, road_y + 42.0, c.z).with_rotation(Quat::from_rotation_y(yaw)),
        RaceEntity,
    ));

    // Deck railings, pillars and slung "cables" along a stretch of the road.
    for j in -6i32..=6 {
        let idx = ((li as i32 + j).rem_euclid(n as i32)) as usize;
        let cc = road_centerline[idx];
        let nn = road_centerline[(idx + 1) % n];
        let d = Vec3::new(nn.x - cc.x, 0.0, nn.z - cc.z).normalize_or_zero();
        let r = Vec3::Y.cross(d).normalize_or_zero();
        let y = get_terrain_height(cc.x, cc.z) + 0.4;
        let yw = d.x.atan2(d.z);
        for side in [-1.0_f32, 1.0] {
            let p = cc + r * (11.0 * side);
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.4, 1.4, 8.0))),
                MeshMaterial3d(steel.clone()),
                Transform::from_xyz(p.x, y + 0.7, p.z).with_rotation(Quat::from_rotation_y(yw)),
                RaceEntity,
            ));
            // Support pillar to the valley floor.
            let ph = (y - low.0).max(3.0) + 6.0;
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(1.4, ph, 1.4))),
                MeshMaterial3d(cable_mat.clone()),
                Transform::from_xyz(p.x, y - ph / 2.0, p.z),
                RaceEntity,
            ));
        }
    }
}

// --- Giant drivable tunnel on the circuit ----------------------------------

fn spawn_tunnel(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    road_centerline: &[Vec3],
) {
    let n = road_centerline.len();
    // Highest point on the actual road → bore a covered tunnel through the hill.
    let mut hi = (f32::MIN, 0usize);
    for (i, c) in road_centerline.iter().enumerate() {
        let h = get_terrain_height(c.x, c.z);
        if h > hi.0 {
            hi = (h, i);
        }
    }

    let wall_mat = materials.add(Color::srgb(0.26, 0.26, 0.3));
    let portal_mat = materials.add(Color::srgb(0.35, 0.35, 0.4));
    let wall_mesh = meshes.add(Cuboid::new(2.0, 10.0, 8.0));
    let roof_mesh = meshes.add(Cuboid::new(28.0, 2.0, 8.0));

    // ~110 m covered corridor centred on the peak (road samples are ~6 m apart).
    let span = 9i32;
    for j in -span..=span {
        let idx = ((hi.1 as i32 + j).rem_euclid(n as i32)) as usize;
        let c = road_centerline[idx];
        let nn = road_centerline[(idx + 1) % n];
        let d = Vec3::new(nn.x - c.x, 0.0, nn.z - c.z).normalize_or_zero();
        let r = Vec3::Y.cross(d).normalize_or_zero();
        let gy = get_terrain_height(c.x, c.z);
        let yaw = d.x.atan2(d.z);
        for side in [-1.0_f32, 1.0] {
            let p = c + r * (12.0 * side);
            commands.spawn((
                Mesh3d(wall_mesh.clone()),
                MeshMaterial3d(wall_mat.clone()),
                Transform::from_xyz(p.x, gy + 5.0, p.z).with_rotation(Quat::from_rotation_y(yaw)),
                RaceEntity,
            ));
        }
        commands.spawn((
            Mesh3d(roof_mesh.clone()),
            MeshMaterial3d(wall_mat.clone()),
            Transform::from_xyz(c.x, gy + 10.0, c.z).with_rotation(Quat::from_rotation_y(yaw)),
            RaceEntity,
        ));

        // Big portal lintel ABOVE each entrance (kept clear of the road opening).
        if j == -span || j == span {
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(34.0, 7.0, 2.0))),
                MeshMaterial3d(portal_mat.clone()),
                Transform::from_xyz(c.x, gy + 14.5, c.z).with_rotation(Quat::from_rotation_y(yaw)),
                RaceEntity,
            ));
        }
    }
}
