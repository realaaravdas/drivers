use bevy::prelude::*;
use rand::RngExt;
use bevy_rapier3d::prelude::*;
use crate::game_state::GameState;

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

    // Ground plane
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::new(Vec3::Y, Default::default()).mesh().size(1000.0, 1000.0))),
        MeshMaterial3d(materials.add(Color::srgb(0.2, 0.2, 0.2))),
        Transform::from_xyz(0.0, -0.1, 0.0),
        Collider::cuboid(500.0, 0.1, 500.0),
    ));

    // A simple loop for the track (square loop for now)
    let min_x = -3;
    let max_x = 3;
    let min_z = -3;
    let max_z = 3;

    let mut waypoints = Vec::new();
    
    // Bottom edge
    for x in min_x..=max_x { waypoints.push(Vec3::new(x as f32 * BLOCK_SIZE, 0.0, min_z as f32 * BLOCK_SIZE)); }
    // Right edge
    for z in (min_z + 1)..=max_z { waypoints.push(Vec3::new(max_x as f32 * BLOCK_SIZE, 0.0, z as f32 * BLOCK_SIZE)); }
    // Top edge
    for x in (min_x..max_x).rev() { waypoints.push(Vec3::new(x as f32 * BLOCK_SIZE, 0.0, max_z as f32 * BLOCK_SIZE)); }
    // Left edge
    for z in (min_z + 1..max_z).rev() { waypoints.push(Vec3::new(min_x as f32 * BLOCK_SIZE, 0.0, z as f32 * BLOCK_SIZE)); }

    level_data.waypoints = waypoints.clone();
    level_data.start_pos = waypoints[0] + Vec3::Y * 2.0;

    // Generate Buildings
    for x in -GRID_SIZE..=GRID_SIZE {
        for z in -GRID_SIZE..=GRID_SIZE {
            let pos = Vec3::new(x as f32 * BLOCK_SIZE, 0.0, z as f32 * BLOCK_SIZE);
            
            // Check if pos is on the track
            let mut is_track = false;
            for wp in &waypoints {
                if wp.distance(pos) < 1.0 {
                    is_track = true;
                    break;
                }
            }

            if !is_track {
                // Spawn building
                let height = rng.random_range(10.0..50.0);
                let color = Color::srgb(rng.random_range(0.3..0.9), rng.random_range(0.3..0.9), rng.random_range(0.3..0.9));
                
                commands.spawn((
                    Mesh3d(meshes.add(Cuboid::new(BLOCK_SIZE - ROAD_WIDTH, height, BLOCK_SIZE - ROAD_WIDTH))),
                    MeshMaterial3d(materials.add(color)),
                    Transform::from_xyz(pos.x, height / 2.0, pos.z),
                    Collider::cuboid((BLOCK_SIZE - ROAD_WIDTH) / 2.0, height / 2.0, (BLOCK_SIZE - ROAD_WIDTH) / 2.0),
                ));
            } else {
                // Spawn a visual waypoint marker (optional, for debugging)
                commands.spawn((
                    Mesh3d(meshes.add(Sphere::new(1.0).mesh().ico(3).unwrap())),
                    MeshMaterial3d(materials.add(Color::srgb(1.0, 0.0, 0.0))),
                    Transform::from_xyz(pos.x, 1.0, pos.z),
                ));
            }
        }
    }

    state.set(GameState::Racing);
}
