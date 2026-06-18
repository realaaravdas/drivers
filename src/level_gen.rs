use bevy::prelude::*;
use bevy::mesh::Indices;
use rand::RngExt;
use bevy_rapier3d::prelude::*;
use crate::game_state::{GameState, RaceEntity};

pub struct LevelGenPlugin;

impl Plugin for LevelGenPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(LevelData::default())
           .add_systems(OnEnter(GameState::GeneratingLevel), generate_level);
    }
}

#[derive(Resource, Default)]
pub struct LevelData {
    pub waypoints: Vec<Vec3>,
    pub start_pos: Vec3,
}

const GRID_SIZE: i32 = 10;
const BLOCK_SIZE: f32 = 40.0;
const ROAD_WIDTH: f32 = 16.0;

pub fn get_terrain_height(x: f32, z: f32) -> f32 {
    // Large rolling hills — two overlapping low-frequency waves, like SF neighborhoods
    let large = (x / 420.0).sin() * (z / 370.0 + 0.6).cos() * 28.0
              + (x / 390.0 + 1.3).cos() * (z / 440.0).sin() * 22.0;
    // Medium hills add local variety
    let medium = ((x + z * 0.4) / 160.0).sin() * 10.0
               + ((z - x * 0.3) / 140.0).cos() * 9.0;
    // Subtle surface texture
    let small = (x / 68.0).sin() * 2.5 + (z / 62.0).cos() * 2.0;
    // +22 baseline keeps most terrain above zero; floor at 0 creates flat valley areas
    (large + medium + small + 22.0).max(0.0)
}

fn generate_level(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut state: ResMut<NextState<GameState>>,
    mut level_data: ResMut<LevelData>,
) {
    let mut rng = rand::rng();

    let num_rows = 401; // Back down a bit to 401x401 to keep generation fast
    let num_cols = 401;
    let grid_size = 8.0; // 3200 total size. 8 unit grid is fine for road
    let total_size = (num_rows - 1) as f32 * grid_size;
    let half_size = total_size / 2.0;

    // 1. Generate waypoints FIRST
    let num_points = rng.random_range(24..40);
    let mut waypoints: Vec<Vec3> = Vec::new();
    
    for i in 0..num_points {
        let angle = (i as f32 / num_points as f32) * std::f32::consts::TAU;
        let radius = rng.random_range(12.0..24.0);
        
        let x = (angle.cos() * radius).round() * 40.0; // scale up
        let z = (angle.sin() * radius).round() * 40.0;
        
        let mut pos = Vec3::new(x, 0.0, z);
        pos.y = get_terrain_height(pos.x, pos.z);
        
        if waypoints.is_empty() || waypoints.last().unwrap().distance(pos) > 1.0 {
            waypoints.push(pos);
        }
    }

    level_data.waypoints = waypoints.clone();
    level_data.start_pos = waypoints[0] + Vec3::Y * 5.0;

    let distance_to_segment = |p: Vec3, a: Vec3, b: Vec3| -> f32 {
        let pa = p - a;
        let ba = b - a;
        let h = (pa.dot(ba) / ba.dot(ba)).clamp(0.0, 1.0);
        (pa - ba * h).length()
    };

    // 2. Generate heightfield and vertex colors
    let mut heights = Vec::with_capacity(num_rows * num_cols);
    let mut positions = Vec::with_capacity(num_rows * num_cols);
    let mut normals = Vec::with_capacity(num_rows * num_cols);
    let mut uvs = Vec::with_capacity(num_rows * num_cols);
    let mut colors = Vec::with_capacity(num_rows * num_cols);

    let mut segments = Vec::new();
    let num_wp = waypoints.len();
    for i in 0..num_wp {
        let wp1 = waypoints[i];
        let wp2 = waypoints[(i + 1) % num_wp];
        let l2 = wp1.distance_squared(wp2);
        let min_x = wp1.x.min(wp2.x) - 16.0;
        let max_x = wp1.x.max(wp2.x) + 16.0;
        let min_z = wp1.z.min(wp2.z) - 16.0;
        let max_z = wp1.z.max(wp2.z) + 16.0;
        segments.push((wp1, wp2, l2, min_x, max_x, min_z, max_z));
    }

    for z in 0..num_cols {
        for x in 0..num_rows {
            let px = x as f32 * grid_size - half_size;
            let pz = z as f32 * grid_size - half_size;
            let h = get_terrain_height(px, pz);
            
            heights.push(h);
            positions.push([px, h, pz]);
            normals.push([0.0, 1.0, 0.0]); // We'll compute real normals later
            uvs.push([x as f32 / num_rows as f32, z as f32 / num_cols as f32]);
            
            let pos = Vec3::new(px, 0.0, pz);
            let mut min_dist = f32::MAX;
            
            for &(wp1, wp2, l2, min_x, max_x, min_z, max_z) in &segments {
                if pos.x < min_x || pos.x > max_x || pos.z < min_z || pos.z > max_z {
                    continue;
                }
                let t = ((pos.x - wp1.x) * (wp2.x - wp1.x) + (pos.z - wp1.z) * (wp2.z - wp1.z)) / l2;
                let t = t.clamp(0.0, 1.0);
                let proj = Vec3::new(wp1.x + t * (wp2.x - wp1.x), 0.0, wp1.z + t * (wp2.z - wp1.z));
                let dist = pos.distance(proj);
                if dist < min_dist {
                    min_dist = dist;
                }
            }

            if min_dist < 3.5 { // Center line — wide enough to be clearly visible at 8m grid
                colors.push([1.0, 0.92, 0.1, 1.0]);
            } else if min_dist < 16.0 { // Road — 16m aligns exactly with 2× grid spacing → sharp edge
                colors.push([0.48, 0.48, 0.48, 1.0]);
            } else { // Grass — high contrast with road, no blend zone
                colors.push([0.12, 0.42, 0.08, 1.0]);
            }
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

    // Heightfield collider: no internal-edge artifacts, no ghost collisions.
    // Rapier's heightfield is purpose-built for terrain — objects glide smoothly over it.
    let collider = Collider::heightfield(
        heights.clone(),
        num_rows,
        num_cols,
        Vec3::new(total_size, 1.0, total_size),
    );

    commands.spawn((
        Mesh3d(meshes.add(terrain_mesh)),
        MeshMaterial3d(materials.add(Color::WHITE)), // White so vertex colors show perfectly
        Transform::IDENTITY,
        collider,
        RaceEntity,
    ));

    // 3. Generate Buildings
    let building_grid = 30; // 60x60 grid
    for x in -building_grid..=building_grid {
        for z in -building_grid..=building_grid {
            let pos_x = x as f32 * BLOCK_SIZE;
            let pos_z = z as f32 * BLOCK_SIZE;
            let mut pos = Vec3::new(pos_x, 0.0, pos_z);
            pos.y = get_terrain_height(pos.x, pos.z);
            
            let mut is_track = false;
            let num_wp = waypoints.len();
            for i in 0..num_wp {
                let wp1 = waypoints[i];
                let wp2 = waypoints[(i + 1) % num_wp];
                
                if distance_to_segment(Vec3::new(pos.x, 0.0, pos.z), Vec3::new(wp1.x, 0.0, wp1.z), Vec3::new(wp2.x, 0.0, wp2.z)) < BLOCK_SIZE * 0.8 {
                    is_track = true;
                    break;
                }
            }

            if !is_track {
                let height = rng.random_range(10.0..50.0);
                let color = Color::srgb(rng.random_range(0.3..0.9), rng.random_range(0.3..0.9), rng.random_range(0.3..0.9));
                
                commands.spawn((
                    Mesh3d(meshes.add(Cuboid::new(BLOCK_SIZE - ROAD_WIDTH, height, BLOCK_SIZE - ROAD_WIDTH))),
                    MeshMaterial3d(materials.add(color)),
                    Transform::from_xyz(pos.x, pos.y + height / 2.0 - 5.0, pos.z),
                    Collider::cuboid((BLOCK_SIZE - ROAD_WIDTH) / 2.0, height / 2.0, (BLOCK_SIZE - ROAD_WIDTH) / 2.0),
                    RaceEntity,
                ));
            }
        }
    }

    // 4. Spawn Ski Gates
    let pole_mesh = meshes.add(Cylinder::new(0.5, 8.0));
    for (i, wp) in waypoints.iter().enumerate() {
        let color = if i == 0 { Color::srgb(1.0, 1.0, 1.0) } else { Color::srgb(1.0, 0.0, 0.0) };
        let mat = materials.add(color);
        
        let dir = if i < waypoints.len() - 1 {
            (waypoints[i+1] - *wp).normalize_or_zero()
        } else {
            (waypoints[0] - *wp).normalize_or_zero()
        };
        let right = Vec3::Y.cross(dir).normalize_or_zero();
        
        commands.spawn((
            Transform::from_translation(*wp),
            GlobalTransform::default(),
            Visibility::default(),
            crate::game_state::WaypointMarker(i),
            RaceEntity,
        )).with_children(|parent| {
            parent.spawn((
                Mesh3d(pole_mesh.clone()),
                MeshMaterial3d(mat.clone()),
                Transform::from_translation(right * 10.0 + Vec3::Y * 4.0),
            ));
            parent.spawn((
                Mesh3d(pole_mesh.clone()),
                MeshMaterial3d(mat.clone()),
                Transform::from_translation(-right * 10.0 + Vec3::Y * 4.0),
            ));
        });
    }

    state.set(GameState::Racing);
}
