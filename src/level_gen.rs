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

fn generate_level(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut state: ResMut<NextState<GameState>>,
    mut level_data: ResMut<LevelData>,
) {
    let mut rng = rand::rng();

    let num_rows = 321;
    let num_cols = 321;
    let grid_size = 10.0;
    let total_size = (num_rows - 1) as f32 * grid_size;
    let half_size = total_size / 2.0;

    let get_height = |x: f32, z: f32| -> f32 {
        let raw_h = (x / 400.0).sin() + (z / 300.0).cos() + (x * z / 80000.0).sin() * 0.5;
        // Smooth the hills, create large flat areas
        (raw_h * raw_h * raw_h) * 20.0
    };

    // 1. Generate waypoints FIRST
    let num_points = rng.random_range(24..40);
    let mut waypoints: Vec<Vec3> = Vec::new();
    
    for i in 0..num_points {
        let angle = (i as f32 / num_points as f32) * std::f32::consts::TAU;
        let radius = rng.random_range(12.0..24.0);
        
        let x = (angle.cos() * radius).round() * 40.0; // scale up
        let z = (angle.sin() * radius).round() * 40.0;
        
        let mut pos = Vec3::new(x, 0.0, z);
        pos.y = get_height(pos.x, pos.z);
        
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

    for z in 0..num_cols {
        for x in 0..num_rows {
            let px = x as f32 * grid_size - half_size;
            let pz = z as f32 * grid_size - half_size;
            let h = get_height(px, pz);
            
            heights.push(h);
            positions.push([px, h, pz]);
            normals.push([0.0, 1.0, 0.0]); 
            uvs.push([x as f32 / num_rows as f32, z as f32 / num_cols as f32]);

            // Road coloring
            let p2d = Vec3::new(px, 0.0, pz);
            let mut min_dist = std::f32::MAX;
            for i in 0..waypoints.len() {
                let wp1 = waypoints[i];
                let wp2 = waypoints[(i + 1) % waypoints.len()];
                let w1_2d = Vec3::new(wp1.x, 0.0, wp1.z);
                let w2_2d = Vec3::new(wp2.x, 0.0, wp2.z);
                
                let dist = distance_to_segment(p2d, w1_2d, w2_2d);
                if dist < min_dist {
                    min_dist = dist;
                }
            }

            if min_dist < 1.0 { // Center line
                colors.push([1.0, 0.9, 0.1, 1.0]); 
            } else if min_dist < 15.0 { // Road
                colors.push([0.3, 0.3, 0.3, 1.0]);
            } else if min_dist < 18.0 { // Edge blend
                let t = (min_dist - 15.0) / 3.0;
                let r = 0.3 * (1.0 - t) + 0.2 * t;
                let g = 0.3 * (1.0 - t) + 0.3 * t;
                let b = 0.3 * (1.0 - t) + 0.2 * t;
                colors.push([r, g, b, 1.0]);
            } else { // Grass
                colors.push([0.2, 0.3, 0.2, 1.0]);
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

    let vertices: Vec<Vec3> = positions.iter().map(|p| Vec3::from(*p)).collect();
    let trimesh_indices: Vec<[u32; 3]> = indices.chunks(3).map(|c| [c[0], c[1], c[2]]).collect();
    let collider = Collider::trimesh(vertices, trimesh_indices).unwrap();

    let mut terrain_mesh = Mesh::new(bevy::render::render_resource::PrimitiveTopology::TriangleList, bevy::asset::RenderAssetUsages::default());
    terrain_mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    terrain_mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    terrain_mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    terrain_mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    terrain_mesh.insert_indices(Indices::U32(indices));

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
            pos.y = get_height(pos.x, pos.z);
            
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
