use bevy::prelude::*;
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

    // Ground plane
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::new(Vec3::Y, Default::default()).mesh().size(1000.0, 1000.0))),
        MeshMaterial3d(materials.add(Color::srgb(0.2, 0.2, 0.2))),
        Transform::from_xyz(0.0, -0.1, 0.0),
        Collider::cuboid(500.0, 0.1, 500.0),
        RaceEntity,
    ));

    let num_points = rng.random_range(12..20);
    let mut waypoints: Vec<Vec3> = Vec::new();
    
    // Generate a random noisy circular path
    for i in 0..num_points {
        let angle = (i as f32 / num_points as f32) * std::f32::consts::TAU;
        let radius = rng.random_range(4.0..8.0); // Size of the track
        
        let x = (angle.cos() * radius).round() as i32;
        let z = (angle.sin() * radius).round() as i32;
        
        let pos = Vec3::new(x as f32 * BLOCK_SIZE, 0.0, z as f32 * BLOCK_SIZE);
        
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
            let pos = Vec3::new(x as f32 * BLOCK_SIZE, 0.0, z as f32 * BLOCK_SIZE);
            
            // Check if pos is on or near the track segments
            let mut is_track = false;
            let num_wp = waypoints.len();
            for i in 0..num_wp {
                let wp1 = waypoints[i];
                let wp2 = waypoints[(i + 1) % num_wp];
                
                if distance_to_segment(pos, wp1, wp2) < BLOCK_SIZE * 0.8 {
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
                    RaceEntity,
                ));
            }
        }
    }

    // Spawn Waypoint Spheres
    for (i, wp) in waypoints.iter().enumerate() {
        let color = if i == 0 { Color::srgb(1.0, 1.0, 1.0) } else { Color::srgb(1.0, 0.0, 0.0) };
        let size = if i == 0 { 2.0 } else { 1.0 }; // Finish line is bigger
        
        commands.spawn((
            Mesh3d(meshes.add(Sphere::new(size).mesh().ico(3).unwrap())),
            MeshMaterial3d(materials.add(color)),
            Transform::from_xyz(wp.x, size, wp.z),
            crate::game_state::WaypointMarker(i),
            RaceEntity,
        ));
    }

    state.set(GameState::Racing);
}
