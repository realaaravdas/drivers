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

    let num_rows = 41;
    let num_cols = 41;
    let grid_size = 40.0;
    let total_size = (num_rows - 1) as f32 * grid_size;
    let half_size = total_size / 2.0;

    let mut heights = Vec::with_capacity(num_rows * num_cols);
    let mut positions = Vec::with_capacity(num_rows * num_cols);
    let mut normals = Vec::with_capacity(num_rows * num_cols);
    let mut uvs = Vec::with_capacity(num_rows * num_cols);

    // Generate heightfield
    for z in 0..num_cols {
        for x in 0..num_rows {
            let px = x as f32 * grid_size - half_size;
            let pz = z as f32 * grid_size - half_size;
            
            // Simple noise using sine waves
            let h = (px / 200.0).sin() * 20.0 + (pz / 150.0).cos() * 15.0 + (px * pz / 10000.0).sin() * 10.0;
            heights.push(h);

            positions.push([px, h, pz]);
            normals.push([0.0, 1.0, 0.0]); // Will compute properly later, or just use up
            uvs.push([x as f32 / num_rows as f32, z as f32 / num_cols as f32]);
        }
    }

    // Compute normals
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
    terrain_mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    terrain_mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    terrain_mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    terrain_mesh.insert_indices(Indices::U32(indices));

    commands.spawn((
        Mesh3d(meshes.add(terrain_mesh)),
        MeshMaterial3d(materials.add(Color::srgb(0.2, 0.3, 0.2))),
        Transform::IDENTITY,
        Collider::heightfield(heights, num_rows, num_cols, Vec3::new(total_size, 1.0, total_size)),
        RaceEntity,
    ));

    // Generate waypoints and map them to the terrain
    let num_points = rng.random_range(12..20);
    let mut waypoints: Vec<Vec3> = Vec::new();
    
    // Function to get height at any world position
    let get_height = |pos: Vec3| -> f32 {
        let x = pos.x;
        let z = pos.z;
        (x / 200.0).sin() * 20.0 + (z / 150.0).cos() * 15.0 + (x * z / 10000.0).sin() * 10.0
    };

    for i in 0..num_points {
        let angle = (i as f32 / num_points as f32) * std::f32::consts::TAU;
        let radius = rng.random_range(4.0..8.0); // Size of the track
        
        let x = (angle.cos() * radius).round() as i32;
        let z = (angle.sin() * radius).round() as i32;
        
        let mut pos = Vec3::new(x as f32 * BLOCK_SIZE, 0.0, z as f32 * BLOCK_SIZE);
        pos.y = get_height(pos);
        
        if waypoints.is_empty() || waypoints.last().unwrap().distance(pos) > 1.0 {
            waypoints.push(pos);
        }
    }

    level_data.waypoints = waypoints.clone();
    level_data.start_pos = waypoints[0] + Vec3::Y * 2.0;

    fn distance_to_segment(p: Vec3, a: Vec3, b: Vec3) -> f32 {
        let pa = p - a;
        let ba = b - a;
        let h = (pa.dot(ba) / ba.dot(ba)).clamp(0.0, 1.0);
        (pa - ba * h).length()
    }

    // Generate Buildings
    for x in -GRID_SIZE..=GRID_SIZE {
        for z in -GRID_SIZE..=GRID_SIZE {
            let mut pos = Vec3::new(x as f32 * BLOCK_SIZE, 0.0, z as f32 * BLOCK_SIZE);
            pos.y = get_height(pos);
            
            let mut is_track = false;
            let num_wp = waypoints.len();
            for i in 0..num_wp {
                let wp1 = waypoints[i];
                let wp2 = waypoints[(i + 1) % num_wp];
                
                // ignore y for track distance
                if distance_to_segment(Vec3::new(pos.x, 0.0, pos.z), Vec3::new(wp1.x, 0.0, wp1.z), Vec3::new(wp2.x, 0.0, wp2.z)) < BLOCK_SIZE * 0.8 {
                    is_track = true;
                    break;
                }
            }

            if !is_track {
                // Spawn building, sinking it slightly into the terrain so it doesn't float
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

    // Spawn Ski Gates
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
