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
    // Diagonal axes — symmetric under x↔z swap so heightfield and visual mesh
    // always agree regardless of Rapier's internal row/column axis convention.
    //   s = x+z : swapping x,z gives z+x = s  (fully symmetric)
    //   d = x-z : swapping gives z-x = -d, but cos(-d) = cos(d)  (even function)
    // Rule: use sin(s), cos(s), cos(d) — never sin(d).
    let s = x + z;
    let d = x - z;

    // Large hills — bigger, more dramatic rolling terrain
    let large = (s / 580.0).sin() * 38.0
              + (d / 630.0).cos() * 34.0;
    // Medium hills — neighbourhood scale
    let medium = (s / 195.0).sin() * 15.0
               + (d / 175.0).cos() * 13.0
               + (s / 115.0).cos() * 6.0;
    // Fine surface texture
    let small = (d / 78.0).cos() * 3.5 + (s / 88.0).sin() * 3.0;

    // Baseline keeps most terrain positive; .max(0) creates flat valleys
    (large + medium + small + 38.0).max(0.0)
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

    // 1. Generate waypoints FIRST — a smooth, organic closed loop. The radius is
    // modulated by a few *integer* harmonics of the loop angle, which keeps it
    // perfectly periodic (no kink at the seam) while giving flowing, realistic
    // curves and straights instead of a jagged polygon.
    let num_points = rng.random_range(30..46);
    let base_radius = rng.random_range(520.0..760.0);
    let tau = std::f32::consts::TAU;
    let (a1, p1) = (rng.random_range(0.08..0.22), rng.random_range(0.0..tau));
    let (a2, p2) = (rng.random_range(0.05..0.16), rng.random_range(0.0..tau));
    let (a3, p3) = (rng.random_range(0.03..0.10), rng.random_range(0.0..tau));

    let mut waypoints: Vec<Vec3> = Vec::new();
    for i in 0..num_points {
        let angle = (i as f32 / num_points as f32) * tau;
        let r = base_radius
            * (1.0
                + a1 * (angle + p1).sin()
                + a2 * (2.0 * angle + p2).sin()
                + a3 * (3.0 * angle + p3).sin());

        let x = angle.cos() * r;
        let z = angle.sin() * r;

        let mut pos = Vec3::new(x, 0.0, z);
        pos.y = get_terrain_height(pos.x, pos.z);
        waypoints.push(pos);
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
            let h = get_terrain_height(px, pz);

            heights.push(h);
            positions.push([px, h, pz]);
            normals.push([0.0, 1.0, 0.0]); // We'll compute real normals later
            uvs.push([x as f32 / num_rows as f32, z as f32 / num_cols as f32]);

            // Grass everywhere — the road is a separate high-resolution ribbon mesh
            // (see `build_road_mesh`) so its lines stay crisp regardless of this
            // coarse 8 m terrain grid. A subtle tint breaks up the flatness.
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

    // 2b. High-resolution road ribbon laid over the terrain. Its own dense mesh
    // gives crisp, high-def lane lines (yellow dashed centre, white edges) that the
    // coarse terrain grid can't. Vertex-coloured, double-sided, no collider.
    let road_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.95,
        cull_mode: None,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(build_road_mesh(&waypoints))),
        MeshMaterial3d(road_mat),
        Transform::IDENTITY,
        RaceEntity,
    ));

    // 3. Generate Buildings — windowed skyscrapers. A shared window texture (with
    // a few colour tints) is UV-tiled per building, so they look detailed without
    // any extra entities.
    let window_tex = images.add(create_window_texture());
    let tints = [
        Color::srgb(0.72, 0.74, 0.80),
        Color::srgb(0.55, 0.60, 0.72),
        Color::srgb(0.80, 0.74, 0.64),
        Color::srgb(0.62, 0.68, 0.62),
        Color::srgb(0.70, 0.66, 0.70),
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

    let footprint = BLOCK_SIZE - ROAD_WIDTH; // 24 m
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
                let height = rng.random_range(15.0..70.0);

                // Tile the window texture: ~4 m per window across and per floor.
                let mut bmesh: Mesh = Cuboid::new(footprint, height, footprint).into();
                let sx = footprint / 4.0;
                let sy = height / 4.0;
                if let Some(VertexAttributeValues::Float32x2(uvs)) =
                    bmesh.attribute_mut(Mesh::ATTRIBUTE_UV_0)
                {
                    for uv in uvs.iter_mut() {
                        uv[0] *= sx;
                        uv[1] *= sy;
                    }
                }

                let mat = building_mats[rng.random_range(0..building_mats.len())].clone();

                commands.spawn((
                    Mesh3d(meshes.add(bmesh)),
                    MeshMaterial3d(mat),
                    Transform::from_xyz(pos.x, pos.y + height / 2.0 - 5.0, pos.z),
                    Collider::cuboid(footprint / 2.0, height / 2.0, footprint / 2.0),
                    RaceEntity,
                ));
            }
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

    // 5. Scenery & traffic: trees, side roads, crosswalks, signals, rivers/bridges,
    // tunnels and ambient NPC cars.
    crate::props::populate_world(&mut commands, &mut meshes, &mut materials, &waypoints);

    state.set(GameState::Racing);
}

/// Builds a crisp road ribbon that follows the waypoint loop. The cross-section
/// has hard colour bands (white edge lines · dark asphalt · dashed yellow centre)
/// with vertices duplicated at each boundary so the lines stay sharp at any zoom.
/// Vertices are draped onto the terrain so the road hugs the hills.
fn build_road_mesh(waypoints: &[Vec3]) -> Mesh {
    let num_wp = waypoints.len();

    let yellow = [1.0, 0.82, 0.0];
    let asphalt = [0.09, 0.09, 0.10];
    let white = [0.9, 0.9, 0.9];

    // (lateral offset from centre, colour, is_centre_line). Boundaries are
    // duplicated (same offset, different colour) so each band is a solid colour.
    // Half-width 12 → a wide 24 m road.
    let cross: [(f32, [f32; 3], bool); 10] = [
        (-12.0, white, false),
        (-10.5, white, false),
        (-10.5, asphalt, false),
        (-1.2, asphalt, false),
        (-1.2, yellow, true),
        (1.2, yellow, true),
        (1.2, asphalt, false),
        (10.5, asphalt, false),
        (10.5, white, false),
        (12.0, white, false),
    ];

    // Sample a Catmull-Rom spline THROUGH the waypoints so the road flows in smooth
    // curves instead of straight chords between control points. The tangent at each
    // sample gives the road's right vector, and arc-length drives the dash pattern.
    let mut stations: Vec<(Vec3, Vec3)> = Vec::new(); // (centre point, right vector)
    let mut dash_flags: Vec<bool> = Vec::new();
    let mut cumulative = 0.0_f32;
    let mut prev_centre: Option<Vec3> = None;
    const DASH_LEN: f32 = 12.0;

    let flat = |v: Vec3| Vec3::new(v.x, 0.0, v.z);
    let cr = |p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, t: f32| -> Vec3 {
        let t2 = t * t;
        let t3 = t2 * t;
        0.5 * ((2.0 * p1)
            + (-p0 + p2) * t
            + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
            + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3)
    };

    for i in 0..num_wp {
        let p0 = flat(waypoints[(i + num_wp - 1) % num_wp]);
        let p1 = flat(waypoints[i]);
        let p2 = flat(waypoints[(i + 1) % num_wp]);
        let p3 = flat(waypoints[(i + 2) % num_wp]);
        let seg_len = p1.distance(p2).max(0.001);
        let steps = (seg_len / 6.0).ceil().max(2.0) as usize;
        for s in 0..steps {
            let t = s as f32 / steps as f32;
            let centre = cr(p0, p1, p2, p3, t);
            let ahead = cr(p0, p1, p2, p3, t + 0.02);
            let dir = flat(ahead - centre).normalize_or_zero();
            let right = Vec3::Y.cross(dir).normalize_or_zero();

            if let Some(prev) = prev_centre {
                cumulative += centre.distance(prev);
            }
            prev_centre = Some(centre);
            stations.push((centre, right));
            dash_flags.push(((cumulative / DASH_LEN) as i32) % 2 == 0);
        }
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
            let y = get_terrain_height(p.x, p.z) + 0.3;
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
