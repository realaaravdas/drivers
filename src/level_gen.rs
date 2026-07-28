use bevy::prelude::*;
use bevy::mesh::{Indices, VertexAttributeValues};
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use rand::RngExt;
use bevy_rapier3d::prelude::*;
use crate::game_state::{GameState, RaceEntity};

pub struct LevelGenPlugin;

impl Plugin for LevelGenPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(LevelData::default())
           // Free-roam island world. (The procedural racing city — `generate_level`
           // — is kept in the code but switched off for now.)
           .add_systems(OnEnter(GameState::GeneratingLevel), generate_island_level);
    }
}

#[derive(Resource, Default)]
pub struct LevelData {
    pub waypoints: Vec<Vec3>,
    pub start_pos: Vec3,
    /// Dense samples of the racing-road centreline (xz, y=0). Used to keep props
    /// off the road and to let the AI follow the actual curve, not just waypoints.
    pub road_centerline: Vec<Vec3>,
    /// Each side-street avenue as a sampled centreline (xz, y=0).
    pub avenues: Vec<Vec<Vec3>>,
}

/// Minimum horizontal distance from a point to the nearest of a set of points.
fn min_dist_to_points(x: f32, z: f32, points: &[Vec3]) -> f32 {
    let mut best = f32::MAX;
    for p in points {
        let dx = p.x - x;
        let dz = p.z - z;
        let d2 = dx * dx + dz * dz;
        if d2 < best {
            best = d2;
        }
    }
    best.sqrt()
}


// --- Réunion island ---------------------------------------------------------
// The island of Réunion: an oval volcanic island rising from the Indian Ocean,
// with two great massifs — Piton des Neiges (the tall central peak + cirques) and
// Piton de la Fournaise (the SE volcano) — a coastal plain, and ocean all around.
// Vertical scale is exaggerated (Forza-Horizon style) for dramatic terrain.
pub const ISLAND_A: f32 = 1500.0; // E–W semi-axis
pub const ISLAND_B: f32 = 1080.0; // N–S semi-axis

/// Irregular coastline radius (≈1.0 at the shore) as a function of bearing.
pub fn island_coast(ang: f32) -> f32 {
    1.0 + 0.07 * (3.0 * ang).sin()
        + 0.05 * (5.0 * ang + 1.3).cos()
        + 0.035 * (7.0 * ang - 0.6).sin()
        + 0.02 * (11.0 * ang).cos()
}

pub fn get_terrain_height(x: f32, z: f32) -> f32 {
    let ang = z.atan2(x);
    let coast = island_coast(ang);
    let r = ((x / ISLAND_A).powi(2) + (z / ISLAND_B).powi(2)).sqrt();
    let land = coast - r; // >0 inland, <0 out to sea

    if land <= 0.0 {
        // Ocean floor: shelves off then flattens to the abyssal plain.
        return (land * 350.0).max(-140.0);
    }

    // 0 at the coast → ~1 deep inland.
    let inland = (land / coast).clamp(0.0, 1.0);

    // Coastal plain rising gently inland.
    let mut h = 10.0 + inland * 80.0;

    // Piton des Neiges massif (tall central peak).
    let d1x = x + 220.0;
    let d1z = z + 120.0;
    h += 540.0 * (-(d1x * d1x + d1z * d1z) / (2.0 * 470.0 * 470.0)).exp();

    // Piton de la Fournaise (SE volcano, a bit lower, broader shield).
    let d2x = x - 640.0;
    let d2z = z - 430.0;
    h += 380.0 * (-(d2x * d2x + d2z * d2z) / (2.0 * 380.0 * 380.0)).exp();

    // Ridges, gorges and cirques (scaled by `inland` so the coast stays smooth).
    let ridges = ((x / 130.0).sin() * (z / 150.0).cos()) * 55.0
        + ((x / 70.0 + z / 80.0).sin()) * 24.0
        + ((x / 300.0 - z / 260.0).cos()) * 40.0;
    h += ridges * 0.6 * inland;

    // Fade to sea level at the very shore so there are beaches, not cliffs.
    let shore = (land * 7.0).clamp(0.0, 1.0);
    (h * shore).max(0.0)
}

/// A small repeating texture of office windows (lit/unlit) so buildings read as
/// detailed skyscrapers instead of flat blocks — at zero extra entity cost.
fn create_window_texture() -> Image {
    let size = 32u32;
    let cell = 4u32; // 4px per window
    let mut data = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let cx = x / cell;
            let cy = y / cell;
            let frame = (x % cell == 0) || (y % cell == 0); // mullions between windows
            let lit = ((cx.wrapping_mul(73) ^ cy.wrapping_mul(151)) % 5) < 2; // ~40% lit
            let (r, g, b) = if frame {
                (26u8, 28, 34)
            } else if lit {
                (255u8, 232, 158)
            } else {
                (70u8, 92, 122)
            };
            data.extend_from_slice(&[r, g, b, 255]);
        }
    }
    let mut image = Image::new(
        Extent3d { width: size, height: size, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        ..default()
    });
    image
}

/// Shared tree meshes/materials so we can plant thousands cheaply, with variety.
struct TreeKit {
    trunk: Handle<Mesh>,
    foliage: [Handle<Mesh>; 3],
    trunk_mat: Handle<StandardMaterial>,
    greens: [Handle<StandardMaterial>; 4],
}

/// Plants one random-species, random-size tree draped on the terrain at `pos.xz`.
fn plant_tree(commands: &mut Commands, kit: &TreeKit, pos: Vec3, rng: &mut impl RngExt) {
    let ty = get_terrain_height(pos.x, pos.z);
    let scale = rng.random_range(0.7..1.9);
    let kind = rng.random_range(0..3);
    let green = kit.greens[rng.random_range(0..kit.greens.len())].clone();
    let fy = if kind == 1 { 5.5 } else { 8.0 }; // round canopy sits a bit lower
    commands.spawn((
        Mesh3d(kit.trunk.clone()),
        MeshMaterial3d(kit.trunk_mat.clone()),
        Transform::from_xyz(pos.x, ty + 2.5 * scale, pos.z).with_scale(Vec3::splat(scale)),
        RaceEntity,
    ));
    commands.spawn((
        Mesh3d(kit.foliage[kind].clone()),
        MeshMaterial3d(green),
        Transform::from_xyz(pos.x, ty + fy * scale, pos.z).with_scale(Vec3::splat(scale)),
        RaceEntity,
    ));
}

/// A cuboid whose UVs are tiled so the shared window texture reads as ~4 m windows.
fn windowed_cuboid(fx: f32, h: f32, fz: f32) -> Mesh {
    let mut m: Mesh = Cuboid::new(fx, h, fz).into();
    if let Some(VertexAttributeValues::Float32x2(uvs)) = m.attribute_mut(Mesh::ATTRIBUTE_UV_0) {
        let (su, sv) = (fx / 4.0, h / 4.0);
        for uv in uvs.iter_mut() {
            uv[0] *= su;
            uv[1] *= sv;
        }
    }
    m
}

/// Ground colour for the island: sand at the shore, forest green rising inland,
/// bare rock on cliffs/high ground, snow on the summit, dark ocean floor below sea.
fn island_color(x: f32, z: f32, h: f32) -> [f32; 4] {
    if h <= 0.0 {
        return [0.04, 0.18, 0.30, 1.0]; // ocean floor (hidden under the water)
    }
    if h < 3.0 {
        return [0.86, 0.80, 0.58, 1.0]; // beach sand
    }
    if h > 470.0 {
        return [0.93, 0.93, 0.95, 1.0]; // snow on Piton des Neiges
    }
    // Slope → bare rock on the steep faces.
    let e = 8.0;
    let dhdx = (get_terrain_height(x + e, z) - get_terrain_height(x - e, z)) / (2.0 * e);
    let dhdz = (get_terrain_height(x, z + e) - get_terrain_height(x, z - e)) / (2.0 * e);
    let slope = (dhdx * dhdx + dhdz * dhdz).sqrt();
    if slope > 0.9 || h > 330.0 {
        return [0.34, 0.31, 0.29, 1.0];
    }
    // Lush green, darkening with altitude.
    let t = (h / 330.0).clamp(0.0, 1.0);
    [0.17 + t * 0.05, 0.5 - t * 0.2, 0.13 + t * 0.02, 1.0]
}

/// Builds the free-roam island of Réunion: island terrain, surrounding ocean, and
/// a road network (coastal ring + interior radial/ring roads), with forests. No
/// buildings, gates, AI or racing logic — those are switched off for now.
fn generate_island_level(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut state: ResMut<NextState<GameState>>,
    mut level_data: ResMut<LevelData>,
) {
    let mut rng = rand::rng();
    let tau = std::f32::consts::TAU;

    // --- Terrain mesh -----------------------------------------------------
    let n = 401usize;
    let grid = 11.0_f32; // ~±2200 span, comfortably larger than the island
    let half = (n as f32 - 1.0) * grid / 2.0;
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(n * n);
    let mut normals: Vec<[f32; 3]> = vec![[0.0, 1.0, 0.0]; n * n];
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(n * n);
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(n * n);
    let mut heights: Vec<f32> = Vec::with_capacity(n * n);
    for zi in 0..n {
        for xi in 0..n {
            let px = xi as f32 * grid - half;
            let pz = zi as f32 * grid - half;
            let h = get_terrain_height(px, pz);
            heights.push(h);
            positions.push([px, h, pz]);
            uvs.push([xi as f32 / n as f32, zi as f32 / n as f32]);
            colors.push(island_color(px, pz, h));
        }
    }
    for zi in 0..n {
        for xi in 0..n {
            let idx = zi * n + xi;
            let mut nx = 0.0;
            let mut nz = 0.0;
            if xi > 0 && xi < n - 1 {
                nx = heights[idx - 1] - heights[idx + 1];
            }
            if zi > 0 && zi < n - 1 {
                nz = heights[(zi - 1) * n + xi] - heights[(zi + 1) * n + xi];
            }
            let nrm = Vec3::new(nx, grid * 2.0, nz).normalize();
            normals[idx] = [nrm.x, nrm.y, nrm.z];
        }
    }
    let mut indices: Vec<u32> = Vec::new();
    for zi in 0..n - 1 {
        for xi in 0..n - 1 {
            let s = (zi * n + xi) as u32;
            let w = n as u32;
            indices.extend_from_slice(&[s, s + w, s + 1, s + 1, s + w, s + 1 + w]);
        }
    }
    let mut terrain = Mesh::new(
        bevy::render::render_resource::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
    );
    terrain.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    terrain.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    terrain.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    terrain.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    terrain.insert_indices(Indices::U32(indices));
    commands.spawn((
        Mesh3d(meshes.add(terrain)),
        MeshMaterial3d(materials.add(Color::WHITE)),
        Transform::IDENTITY,
        RaceEntity,
    ));

    // --- Ocean ------------------------------------------------------------
    let ocean_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.05, 0.24, 0.42),
        perceptual_roughness: 0.2,
        metallic: 0.15,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(6400.0, 4.0, 6400.0))),
        MeshMaterial3d(ocean_mat),
        Transform::from_xyz(0.0, -2.0, 0.0), // surface at y = 0
        RaceEntity,
    ));

    // --- Coastal ring road (main road + the map "loop") -------------------
    let num_ring = 72;
    let mut ring: Vec<Vec3> = Vec::new();
    for i in 0..num_ring {
        let th = i as f32 / num_ring as f32 * tau;
        let rho = 0.87 * island_coast(th);
        let x = ISLAND_A * rho * th.cos();
        let z = ISLAND_B * rho * th.sin();
        ring.push(Vec3::new(x, get_terrain_height(x, z), z));
    }
    let ring_cl = sample_road_centerline(&ring);
    let road_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.95,
        cull_mode: None,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(build_road_mesh(&ring_cl))),
        MeshMaterial3d(road_mat),
        Transform::IDENTITY,
        RaceEntity,
    ));
    level_data.waypoints = ring.clone();
    level_data.road_centerline = ring_cl.clone();
    level_data.avenues = Vec::new();
    level_data.start_pos = ring[0] + Vec3::Y * 3.0;

    // --- Secondary roads: radials into the interior + inner ring roads ----
    let avenue_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.95,
        cull_mode: None,
        ..default()
    });
    let radials = 14;
    for k in 0..radials {
        let th = k as f32 / radials as f32 * tau + 0.15;
        let mut pts = Vec::new();
        let steps = 46;
        for j in 0..=steps {
            let t = j as f32 / steps as f32;
            let rho = 0.85 * island_coast(th) * (1.0 - t) + 0.16 * t;
            let a = th + (t * 8.0).sin() * 0.03; // gentle wind up the mountain
            let x = ISLAND_A * rho * a.cos();
            let z = ISLAND_B * rho * a.sin();
            pts.push(Vec3::new(x, 0.0, z));
        }
        commands.spawn((
            Mesh3d(meshes.add(build_avenue_mesh(&pts))),
            MeshMaterial3d(avenue_mat.clone()),
            Transform::IDENTITY,
            RaceEntity,
        ));
    }
    for &rr in &[0.58_f32, 0.34] {
        let mut pts = Vec::new();
        let steps = 90;
        for j in 0..=steps {
            let th = j as f32 / steps as f32 * tau;
            let rho = rr * island_coast(th);
            let x = ISLAND_A * rho * th.cos();
            let z = ISLAND_B * rho * th.sin();
            pts.push(Vec3::new(x, 0.0, z));
        }
        commands.spawn((
            Mesh3d(meshes.add(build_avenue_mesh(&pts))),
            MeshMaterial3d(avenue_mat.clone()),
            Transform::IDENTITY,
            RaceEntity,
        ));
    }

    // --- Forests ----------------------------------------------------------
    let tree_kit = TreeKit {
        trunk: meshes.add(Cylinder::new(0.35, 5.0)),
        foliage: [
            meshes.add(Cone { radius: 2.8, height: 8.0 }),
            meshes.add(Sphere::new(3.0)),
            meshes.add(Cone { radius: 1.7, height: 10.5 }),
        ],
        trunk_mat: materials.add(Color::srgb(0.3, 0.2, 0.1)),
        greens: [
            materials.add(Color::srgb(0.1, 0.4, 0.12)),
            materials.add(Color::srgb(0.15, 0.48, 0.15)),
            materials.add(Color::srgb(0.09, 0.34, 0.14)),
            materials.add(Color::srgb(0.2, 0.52, 0.18)),
        ],
    };
    let mut tx = -2000.0_f32;
    while tx < 2000.0 {
        tx += rng.random_range(26.0..46.0);
        let mut tz = -2000.0_f32;
        while tz < 2000.0 {
            tz += rng.random_range(26.0..46.0);
            let h = get_terrain_height(tx, tz);
            if h < 6.0 || h > 300.0 {
                continue; // no trees on the beach, in the sea, or high on the rock
            }
            if min_dist_to_points(tx, tz, &ring_cl) < 14.0 {
                continue; // keep the main road clear
            }
            if rng.random_range(0.0..1.0) < 0.55 {
                continue;
            }
            let jx = tx + rng.random_range(-9.0..9.0);
            let jz = tz + rng.random_range(-9.0..9.0);
            plant_tree(&mut commands, &tree_kit, Vec3::new(jx, 0.0, jz), &mut rng);
        }
    }

    state.set(GameState::Racing);
}

/// Procedural racing-city generator. Kept for reference but currently switched
/// off in favour of `generate_island_level`.
#[allow(dead_code)]
fn generate_level(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut state: ResMut<NextState<GameState>>,
    mut level_data: ResMut<LevelData>,
) {
    let mut rng = rand::rng();

    let num_rows = 401; // Back down a bit to 401x401 to keep generation fast
    let num_cols = 401;
    let grid_size = 8.0; // 3200 total size. 8 unit grid is fine for road
    let total_size = (num_rows - 1) as f32 * grid_size;
    let half_size = total_size / 2.0;

    // 1. Generate waypoints FIRST. Same jagged, varied layout as the classic track
    // (random radius per point, snapped to the 40 m city-block grid), which the
    // Catmull-Rom road builder then rounds into flowing curves — "the old track,
    // but with the sharp corners smoothed."
    let num_points = rng.random_range(24..40);
    let mut waypoints: Vec<Vec3> = Vec::new();
    for i in 0..num_points {
        let angle = (i as f32 / num_points as f32) * std::f32::consts::TAU;
        let radius = rng.random_range(12.0..24.0);

        let x = (angle.cos() * radius).round() * 40.0;
        let z = (angle.sin() * radius).round() * 40.0;

        let mut pos = Vec3::new(x, 0.0, z);
        pos.y = get_terrain_height(pos.x, pos.z);

        // Skip duplicates/too-close points so the spline stays well behaved.
        if waypoints.is_empty() || waypoints.last().unwrap().distance(pos) > 20.0 {
            waypoints.push(pos);
        }
    }

    level_data.waypoints = waypoints.clone();
    level_data.start_pos = waypoints[0] + Vec3::Y * 5.0;

    // Dense racing-road centreline (shared by the road mesh, AI path-following and
    // obstruction clearing) and a varied network of side-street avenues.
    let road_centerline = sample_road_centerline(&waypoints);
    // Side-street avenues temporarily disabled (set back to generate_avenues(&mut rng)
    // to restore). Buildings/props behave fine with an empty network.
    let avenues: Vec<Vec<Vec3>> = Vec::new();
    level_data.road_centerline = road_centerline.clone();
    level_data.avenues = avenues.clone();

    let dist_to_road = |x: f32, z: f32| min_dist_to_points(x, z, &road_centerline);
    let dist_to_avenue = |x: f32, z: f32| {
        avenues
            .iter()
            .map(|av| min_dist_to_points(x, z, av))
            .fold(f32::MAX, f32::min)
    };

    // 2. Generate heightfield and vertex colors
    let mut heights = Vec::with_capacity(num_rows * num_cols);
    let mut positions = Vec::with_capacity(num_rows * num_cols);
    let mut normals = Vec::with_capacity(num_rows * num_cols);
    let mut uvs = Vec::with_capacity(num_rows * num_cols);
    let mut colors = Vec::with_capacity(num_rows * num_cols);

    for z in 0..num_cols {
        for x in 0..num_rows {
            let px = x as f32 * grid_size - half_size;
            let pz = z as f32 * grid_size - half_size;
            let h = get_terrain_height(px, pz);

            heights.push(h);
            positions.push([px, h, pz]);
            normals.push([0.0, 1.0, 0.0]); // We'll compute real normals later
            uvs.push([x as f32 / num_rows as f32, z as f32 / num_cols as f32]);

            // Ground is grass; streets are real ribbon meshes on top (see avenues
            // and the racing road) rather than faint marks baked into this coarse grid.
            let tint = 0.03 * ((px * 0.03).sin() * (pz * 0.037).cos());
            colors.push([0.17 + tint, 0.42 + tint, 0.10, 1.0]);
        }
    }

    for z in 0..num_cols {
        for x in 0..num_rows {
            let idx = z * num_rows + x;
            let mut nx = 0.0;
            let mut nz = 0.0;
            if x > 0 && x < num_rows - 1 {
                nx = heights[idx - 1] - heights[idx + 1];
            }
            if z > 0 && z < num_cols - 1 {
                nz = heights[(z - 1) * num_rows + x] - heights[(z + 1) * num_rows + x];
            }
            let n = Vec3::new(nx, grid_size * 2.0, nz).normalize();
            normals[idx] = [n.x, n.y, n.z];
        }
    }

    let mut indices = Vec::new();
    for z in 0..num_cols - 1 {
        for x in 0..num_rows - 1 {
            let start = (z * num_rows + x) as u32;
            indices.push(start);
            indices.push(start + num_rows as u32);
            indices.push(start + 1);

            indices.push(start + 1);
            indices.push(start + num_rows as u32);
            indices.push(start + 1 + num_rows as u32);
        }
    }

    let mut terrain_mesh = Mesh::new(bevy::render::render_resource::PrimitiveTopology::TriangleList, bevy::asset::RenderAssetUsages::default());
    terrain_mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions.clone());
    terrain_mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    terrain_mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    terrain_mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    terrain_mesh.insert_indices(Indices::U32(indices.clone()));

    // No terrain collider on purpose: cars ride the smooth *analytic* surface via
    // `vehicle::apply_terrain_follow` instead of colliding with this faceted mesh.
    // A rigid box on an 8 m-faceted heightfield catches on every triangle edge
    // (the "imaginary bumps"); following the continuous height function is stutter
    // free. Buildings and cars keep their own colliders, so obstacles still work.
    commands.spawn((
        Mesh3d(meshes.add(terrain_mesh)),
        MeshMaterial3d(materials.add(Color::WHITE)), // White so vertex colors show perfectly
        Transform::IDENTITY,
        RaceEntity,
    ));

    // 2b. High-resolution racing-road ribbon (crisp lane lines), and the network
    // of side-street avenues — all real ribbon meshes draped on the terrain.
    let road_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.95,
        cull_mode: None,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(build_road_mesh(&road_centerline))),
        MeshMaterial3d(road_mat),
        Transform::IDENTITY,
        RaceEntity,
    ));

    let avenue_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.95,
        cull_mode: None,
        ..default()
    });
    for av in avenues.iter() {
        commands.spawn((
            Mesh3d(meshes.add(build_avenue_mesh(av))),
            MeshMaterial3d(avenue_mat.clone()),
            Transform::IDENTITY,
            RaceEntity,
        ));
    }

    // 3. City blocks. Buildings are windowed towers whose height rises toward
    // downtown (map centre). We clear anything that would sit on the racing road
    // or an avenue, leave some empty lots and green parks for variety, and plant
    // park trees.
    let window_tex = images.add(create_window_texture());
    let tints = [
        Color::srgb(0.72, 0.74, 0.80),
        Color::srgb(0.55, 0.60, 0.72),
        Color::srgb(0.80, 0.74, 0.64),
        Color::srgb(0.62, 0.68, 0.62),
        Color::srgb(0.70, 0.66, 0.70),
        Color::srgb(0.5, 0.52, 0.6),
    ];
    let building_mats: Vec<Handle<StandardMaterial>> = tints
        .iter()
        .map(|c| {
            materials.add(StandardMaterial {
                base_color: *c,
                base_color_texture: Some(window_tex.clone()),
                emissive_texture: Some(window_tex.clone()),
                emissive: LinearRgba::rgb(0.45, 0.45, 0.4), // makes the lit windows glow
                perceptual_roughness: 0.8,
                ..default()
            })
        })
        .collect();

    // Several tree species for variety.
    let tree_kit = TreeKit {
        trunk: meshes.add(Cylinder::new(0.35, 5.0)),
        foliage: [
            meshes.add(Cone { radius: 2.8, height: 8.0 }),  // 0: pine
            meshes.add(Sphere::new(3.0)),                   // 1: round / oak
            meshes.add(Cone { radius: 1.7, height: 10.5 }), // 2: tall cypress
        ],
        trunk_mat: materials.add(Color::srgb(0.32, 0.2, 0.1)),
        greens: [
            materials.add(Color::srgb(0.12, 0.42, 0.12)),
            materials.add(Color::srgb(0.18, 0.5, 0.16)),
            materials.add(Color::srgb(0.1, 0.34, 0.14)),
            materials.add(Color::srgb(0.24, 0.56, 0.2)),
        ],
    };

    // Rooftop detail materials.
    let roof_mat = materials.add(Color::srgb(0.15, 0.15, 0.17));
    let antenna_mesh = meshes.add(Cylinder::new(0.3, 12.0));

    // Parks scattered through the city.
    let parks: Vec<Vec2> = (0..8)
        .map(|_| Vec2::new(rng.random_range(-800.0..800.0), rng.random_range(-800.0..800.0)))
        .collect();
    let park_radius = 75.0;

    let building_grid = 28; // ~57x57 blocks
    let block = 40.0_f32;
    let building_groups =
        CollisionGroups::new(crate::vehicle::GROUP_WORLD, crate::vehicle::GROUP_CAR);

    for gx in -building_grid..=building_grid {
        for gz in -building_grid..=building_grid {
            // Heavy jitter + a random rotation break the regular grid into an
            // organic, Amsterdam-ish tangle of blocks and alleys.
            let px = gx as f32 * block + rng.random_range(-11.0..11.0);
            let pz = gz as f32 * block + rng.random_range(-11.0..11.0);

            // Keep the racing road (and avenues, if any) completely clear.
            if dist_to_road(px, pz) < 22.0 || dist_to_avenue(px, pz) < 14.0 {
                continue;
            }

            // Parks: green blocks with tree clusters.
            if parks.iter().any(|p| p.distance(Vec2::new(px, pz)) < park_radius) {
                for _ in 0..3 {
                    let tx = px + rng.random_range(-16.0..16.0);
                    let tz = pz + rng.random_range(-16.0..16.0);
                    if dist_to_road(tx, tz) < 16.0 {
                        continue;
                    }
                    plant_tree(&mut commands, &tree_kit, Vec3::new(tx, 0.0, tz), &mut rng);
                }
                continue;
            }

            // Empty lots: sometimes a tree instead of a building.
            if rng.random_range(0.0..1.0) < 0.12 {
                if rng.random_range(0.0..1.0) < 0.6 {
                    plant_tree(&mut commands, &tree_kit, Vec3::new(px, 0.0, pz), &mut rng);
                }
                continue;
            }

            // TALL city. Height rises steeply toward downtown (map centre).
            let dist_center = (px * px + pz * pz).sqrt();
            let downtown = (1.0 - dist_center / 850.0).clamp(0.0, 1.0);
            let height =
                rng.random_range(24.0..60.0) + downtown * downtown * rng.random_range(50.0..170.0);
            let fx = rng.random_range(12.0..26.0);
            let fz = rng.random_range(12.0..26.0);
            let angle = rng.random_range(0.0..std::f32::consts::TAU);
            let rot = Quat::from_rotation_y(angle);
            let pos_y = get_terrain_height(px, pz);
            let mat = building_mats[rng.random_range(0..building_mats.len())].clone();

            // Main body (windowed).
            let base_y = pos_y + height / 2.0 - 5.0;
            commands.spawn((
                Mesh3d(meshes.add(windowed_cuboid(fx, height, fz))),
                MeshMaterial3d(mat.clone()),
                Transform::from_xyz(px, base_y, pz).with_rotation(rot),
                Collider::cuboid(fx / 2.0, height / 2.0, fz / 2.0),
                building_groups,
                RaceEntity,
            ));

            // Rooftop design variety.
            let top_y = pos_y + height - 5.0;
            let arch = rng.random_range(0.0..1.0);
            if arch < 0.3 {
                // Setback storey.
                let th = height * rng.random_range(0.2..0.45);
                let (tfx, tfz) = (fx * 0.62, fz * 0.62);
                commands.spawn((
                    Mesh3d(meshes.add(windowed_cuboid(tfx, th, tfz))),
                    MeshMaterial3d(mat.clone()),
                    Transform::from_xyz(px, top_y + th / 2.0, pz).with_rotation(rot),
                    RaceEntity,
                ));
            } else if arch < 0.52 {
                // Rooftop mechanical block + antenna spire.
                commands.spawn((
                    Mesh3d(meshes.add(Cuboid::new(fx * 0.45, 3.0, fz * 0.45))),
                    MeshMaterial3d(roof_mat.clone()),
                    Transform::from_xyz(px, top_y + 1.5, pz).with_rotation(rot),
                    RaceEntity,
                ));
                commands.spawn((
                    Mesh3d(antenna_mesh.clone()),
                    MeshMaterial3d(roof_mat.clone()),
                    Transform::from_xyz(px, top_y + 6.0, pz),
                    RaceEntity,
                ));
            }
        }
    }

    // Forests: scatter trees densely everywhere OUTSIDE the city core (hills,
    // fields), clear of the racing road, so greenery is all over the map.
    let city_edge = building_grid as f32 * block;
    let mut ox = -1500.0_f32;
    while ox < 1500.0 {
        ox += rng.random_range(24.0..44.0);
        let mut oz = -1500.0_f32;
        while oz < 1500.0 {
            oz += rng.random_range(24.0..44.0);
            if ox.abs() < city_edge && oz.abs() < city_edge {
                continue; // inside the built-up core
            }
            if dist_to_road(ox, oz) < 16.0 {
                continue;
            }
            if rng.random_range(0.0..1.0) < 0.55 {
                continue;
            }
            let jx = ox + rng.random_range(-10.0..10.0);
            let jz = oz + rng.random_range(-10.0..10.0);
            plant_tree(&mut commands, &tree_kit, Vec3::new(jx, 0.0, jz), &mut rng);
        }
    }

    // 4. Spawn gate markers. The gate "pillars" are two rising columns of coloured
    // smoke (see vehicle::emit_gate_smoke) rather than solid poles, so the marker
    // entity itself is just a transform tag.
    for (i, wp) in waypoints.iter().enumerate() {
        commands.spawn((
            Transform::from_translation(*wp),
            crate::game_state::WaypointMarker(i),
            RaceEntity,
        ));
    }

    // 5. Scenery & traffic: trees, signals, giant bridge/tunnel on the circuit,
    // lakes and ambient NPC cars — all placed clear of the racing road.
    crate::props::populate_world(
        &mut commands,
        &mut meshes,
        &mut materials,
        &waypoints,
        &road_centerline,
        &avenues,
    );

    state.set(GameState::Racing);
}

/// Samples a Catmull-Rom spline through the waypoints into a dense, closed
/// centreline (xz, y=0) — the true racing line the road, AI and obstruction
/// checks all share.
pub fn sample_road_centerline(waypoints: &[Vec3]) -> Vec<Vec3> {
    let num_wp = waypoints.len();
    let flat = |v: Vec3| Vec3::new(v.x, 0.0, v.z);
    let cr = |p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, t: f32| -> Vec3 {
        let t2 = t * t;
        let t3 = t2 * t;
        0.5 * ((2.0 * p1)
            + (-p0 + p2) * t
            + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
            + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3)
    };
    let mut out = Vec::new();
    for i in 0..num_wp {
        let p0 = flat(waypoints[(i + num_wp - 1) % num_wp]);
        let p1 = flat(waypoints[i]);
        let p2 = flat(waypoints[(i + 1) % num_wp]);
        let p3 = flat(waypoints[(i + 2) % num_wp]);
        let seg_len = p1.distance(p2).max(0.001);
        let steps = (seg_len / 6.0).ceil().max(2.0) as usize;
        for s in 0..steps {
            let t = s as f32 / steps as f32;
            // Blend the smooth spline toward the straight chord so corners stay
            // crisp and turns feel sharp, while straights/gentle bends stay smooth.
            let smooth = cr(p0, p1, p2, p3, t);
            let chord = p1.lerp(p2, t);
            out.push(smooth.lerp(chord, 0.4));
        }
    }
    out
}

/// A varied network of straight side-street avenues (varied spacing + a couple of
/// diagonals), each returned as a sampled centreline (xz, y=0).
#[allow(dead_code)]
fn generate_avenues(rng: &mut impl RngExt) -> Vec<Vec<Vec3>> {
    let mut avenues = Vec::new();
    let extent = 1300.0;
    let sample = |a: Vec3, dir: Vec3| -> Vec<Vec3> {
        let mut line = Vec::new();
        let mut t = -extent;
        while t <= extent {
            let p = a + dir * t;
            line.push(Vec3::new(p.x, 0.0, p.z));
            t += 16.0;
        }
        line
    };

    // Horizontal avenues (constant z, varied spacing).
    let mut z = -1150.0;
    while z < 1150.0 {
        z += rng.random_range(120.0..260.0);
        avenues.push(sample(Vec3::new(0.0, 0.0, z), Vec3::X));
    }
    // Vertical avenues (constant x, varied spacing).
    let mut x = -1150.0;
    while x < 1150.0 {
        x += rng.random_range(120.0..260.0);
        avenues.push(sample(Vec3::new(x, 0.0, 0.0), Vec3::Z));
    }
    // A couple of diagonal boulevards for variety.
    for _ in 0..2 {
        let angle = rng.random_range(0.35_f32..1.2);
        let dir = Vec3::new(angle.cos(), 0.0, angle.sin());
        let perp = Vec3::Y.cross(dir).normalize_or_zero();
        let off = perp * rng.random_range(-450.0..450.0);
        avenues.push(sample(off, dir));
    }
    avenues
}

/// Builds a plain draped asphalt avenue (open strip) with a dashed yellow centre
/// line and white edges.
#[allow(dead_code)]
fn build_avenue_mesh(centerline: &[Vec3]) -> Mesh {
    let yellow = [0.9, 0.75, 0.0];
    let asphalt = [0.1, 0.1, 0.11];
    let white = [0.8, 0.8, 0.82];
    let cross: [(f32, [f32; 3], bool); 10] = [
        (-7.0, white, false),
        (-6.0, white, false),
        (-6.0, asphalt, false),
        (-0.4, asphalt, false),
        (-0.4, yellow, true),
        (0.4, yellow, true),
        (0.4, asphalt, false),
        (6.0, asphalt, false),
        (6.0, white, false),
        (7.0, white, false),
    ];

    let n = centerline.len();
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut cumulative = 0.0_f32;
    let w = cross.len();

    for i in 0..n {
        let centre = centerline[i];
        let prev = centerline[i.saturating_sub(1)];
        let next = centerline[(i + 1).min(n - 1)];
        let dir = Vec3::new(next.x - prev.x, 0.0, next.z - prev.z).normalize_or_zero();
        let right = Vec3::Y.cross(dir).normalize_or_zero();
        cumulative += centre.distance(prev);
        let dash_on = ((cumulative / 10.0) as i32) % 2 == 0;
        for (offset, base_color, is_centre) in cross.iter() {
            let p = centre + right * *offset;
            let y = get_terrain_height(p.x, p.z) + 0.05;
            positions.push([p.x, y, p.z]);
            normals.push([0.0, 1.0, 0.0]);
            uvs.push([0.0, 0.0]);
            let col = if *is_centre && !dash_on { asphalt } else { *base_color };
            colors.push([col[0], col[1], col[2], 1.0]);
        }
    }

    let mut indices: Vec<u32> = Vec::new();
    for i in 0..n.saturating_sub(1) {
        for k in 0..(w - 1) {
            let a = (i * w + k) as u32;
            let b = (i * w + k + 1) as u32;
            let c = ((i + 1) * w + k) as u32;
            let d = ((i + 1) * w + k + 1) as u32;
            indices.extend_from_slice(&[a, c, b, b, c, d]);
        }
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

/// Builds a crisp road ribbon along a centreline. The cross-section has hard
/// colour bands (white edge lines · dark asphalt · dashed yellow centre) with
/// vertices duplicated at each boundary so the lines stay sharp. Draped on terrain.
fn build_road_mesh(centerline: &[Vec3]) -> Mesh {
    let yellow = [1.0, 0.82, 0.0];
    let asphalt = [0.09, 0.09, 0.10];
    let white = [0.9, 0.9, 0.9];

    // (lateral offset from centre, colour, is_centre_line). Half-width 10 → 20 m.
    let cross: [(f32, [f32; 3], bool); 10] = [
        (-10.0, white, false),
        (-8.5, white, false),
        (-8.5, asphalt, false),
        (-1.2, asphalt, false),
        (-1.2, yellow, true),
        (1.2, yellow, true),
        (1.2, asphalt, false),
        (8.5, asphalt, false),
        (8.5, white, false),
        (10.0, white, false),
    ];

    let n = centerline.len();
    let mut stations: Vec<(Vec3, Vec3)> = Vec::new(); // (centre, right)
    let mut dash_flags: Vec<bool> = Vec::new();
    let mut cumulative = 0.0_f32;
    const DASH_LEN: f32 = 12.0;
    for i in 0..n {
        let centre = centerline[i];
        let next = centerline[(i + 1) % n];
        let prev = centerline[(i + n - 1) % n];
        let dir = Vec3::new(next.x - prev.x, 0.0, next.z - prev.z).normalize_or_zero();
        let right = Vec3::Y.cross(dir).normalize_or_zero();
        cumulative += centre.distance(prev);
        stations.push((centre, right));
        dash_flags.push(((cumulative / DASH_LEN) as i32) % 2 == 0);
    }
    let num_stations = stations.len();

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();

    for (si, (centre, right)) in stations.iter().enumerate() {
        let dash_on = dash_flags[si];
        for (offset, base_color, is_centre) in cross.iter() {
            let p = *centre + *right * *offset;
            // Hugs the ground (just enough above avenues to overlay them cleanly at
            // crossings without z-fighting).
            let y = get_terrain_height(p.x, p.z) + 0.14;
            positions.push([p.x, y, p.z]);
            normals.push([0.0, 1.0, 0.0]);
            uvs.push([0.0, 0.0]);
            // Centre line disappears in the dash gaps.
            let col = if *is_centre && !dash_on { asphalt } else { *base_color };
            colors.push([col[0], col[1], col[2], 1.0]);
        }
    }

    let w = cross.len();
    let mut indices: Vec<u32> = Vec::new();
    for si in 0..num_stations {
        let ni = (si + 1) % num_stations; // wrap to close the loop
        for k in 0..(w - 1) {
            let a = (si * w + k) as u32;
            let b = (si * w + k + 1) as u32;
            let c = (ni * w + k) as u32;
            let d = (ni * w + k + 1) as u32;
            indices.push(a);
            indices.push(c);
            indices.push(b);
            indices.push(b);
            indices.push(c);
            indices.push(d);
        }
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
