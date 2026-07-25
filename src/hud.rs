use bevy::prelude::*;
use bevy_camera::Viewport;
use bevy_camera::visibility::RenderLayers;
use bevy_camera::{OrthographicProjection, ScalingMode};
use crate::game_state::{GameState, LapTracker, RaceResults, RaceResultRow};
use crate::vehicle::{Vehicle, Player};
use crate::level_gen::LevelData;

/// Cached minimap framing so marker sizes match the zoomed-out full-track view.
#[derive(Resource)]
struct MinimapView {
    extent: f32,
}

/// Centre (XZ) and half-extent of the track, for framing the minimap.
fn track_bounds(waypoints: &[Vec3]) -> (Vec3, f32) {
    if waypoints.is_empty() {
        return (Vec3::ZERO, 500.0);
    }
    let mut center = Vec3::ZERO;
    for wp in waypoints {
        center += Vec3::new(wp.x, 0.0, wp.z);
    }
    center /= waypoints.len() as f32;
    let mut extent = 0.0_f32;
    for wp in waypoints {
        let d = Vec3::new(wp.x, 0.0, wp.z).distance(center);
        extent = extent.max(d);
    }
    (center, extent * 1.15 + 30.0) // margin so the loop isn't flush to the edge
}

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Racing), setup_hud)
           .add_systems(Update, (
               update_place_and_hud,
               update_tesla_hud,
               update_gauge,
               add_minimap_markers,
               update_minimap,
           ).run_if(in_state(GameState::Racing)))
           .add_systems(OnExit(GameState::Racing), cleanup_hud)
           .add_systems(OnEnter(GameState::Scoreboard), setup_scoreboard)
           .add_systems(Update, scoreboard_interaction.run_if(in_state(GameState::Scoreboard)))
           .add_systems(OnExit(GameState::Scoreboard), cleanup_scoreboard)
           // "Continue" routes here; start a fresh race so it isn't a dead end.
           .add_systems(OnEnter(GameState::PostRace), start_next_race);
    }
}

#[derive(Component)]
struct HudEntity;

#[derive(Component)]
struct MinimapCamera;

#[derive(Component)]
struct PlaceText;

#[derive(Component)]
struct TimeText;

#[derive(Component)]
struct ScoreboardEntity;

#[derive(Component)]
enum ScoreboardBtn { MainMenu, Continue }

#[derive(Component)]
struct MinimapMarker {
    target: Entity,
}

#[derive(Component)]
struct TeslaSpeedText;

#[derive(Component)]
struct TeslaRegenBar;

#[derive(Component)]
struct TeslaPowerBar;

#[derive(Component)]
struct TeslaDriftInd;

#[derive(Component)]
struct TeslaBrakeInd;

/// One lit-up dot of the circular speed gauge; `.0` is its index around the arc.
#[derive(Component)]
struct GaugeSegment(usize);

#[derive(Component)]
struct GaugeSpeedText;

const GAUGE_N: usize = 32; // dots around the gauge
const GAUGE_MAX_MPH: f32 = 200.0; // full-scale reading

fn setup_hud(
    mut commands: Commands,
    level_data: Res<LevelData>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    windows: Query<&Window>,
) {
    let window = windows.single().unwrap();
    let width = window.resolution.physical_width();
    let height = window.resolution.physical_height();

    // HUD Container
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(20.0),
            left: Val::Px(20.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(15.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)), // Add dark background
        HudEntity,
    )).with_children(|parent| {
        parent.spawn((
            Text::new("Place: 1st / 1"),
            TextFont { font_size: 40.0, ..default() },
            TextColor(Color::srgb(1.0, 0.8, 0.0)),
            PlaceText,
        ));
        parent.spawn((
            Text::new("Time: 00:00.00"),
            TextFont { font_size: 30.0, ..default() },
            TextColor(Color::WHITE),
            TimeText,
        ));
    });

    // Tesla-style HUD Container (Bottom-Left)
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(20.0),
            left: Val::Px(20.0),
            width: Val::Px(300.0),
            height: Val::Px(140.0),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::SpaceBetween,
            padding: UiRect::all(Val::Px(15.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.05, 0.05, 0.05, 0.8)),
        HudEntity,
    )).with_children(|parent| {
        // Speed Display
        parent.spawn((
            Text::new("0 MPH"),
            TextFont { font_size: 48.0, ..default() },
            TextColor(Color::WHITE),
            TeslaSpeedText,
        ));

        // Energy Bar Container
        parent.spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Px(12.0),
            margin: UiRect::vertical(Val::Px(10.0)),
            ..default()
        }).with_children(|bar_parent| {
            // Background line (gray)
            bar_parent.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.3, 0.3, 0.3)),
            )).with_children(|bg| {
                // Regen Bar (Left side)
                bg.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        right: Val::Percent(50.0),
                        top: Val::Px(0.0),
                        height: Val::Percent(100.0),
                        width: Val::Percent(0.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.0, 1.0, 0.0)), // Green for regen
                    TeslaRegenBar,
                ));
                // Power Bar (Right side)
                bg.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Percent(50.0),
                        top: Val::Px(0.0),
                        height: Val::Percent(100.0),
                        width: Val::Percent(0.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.1, 0.1, 0.1)), // Black/Dark for power
                    TeslaPowerBar,
                ));
                // Center Notch
                bg.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Percent(49.5),
                        width: Val::Percent(1.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(Color::WHITE),
                ));
            });
        });

        // Indicators Row
        parent.spawn(Node {
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            width: Val::Percent(100.0),
            ..default()
        }).with_children(|row| {
            row.spawn((
                Text::new("(P) E-BRAKE"),
                TextFont { font_size: 16.0, ..default() },
                TextColor(Color::srgb(0.3, 0.3, 0.3)), // Dim default
                TeslaDriftInd,
            ));
            row.spawn((
                Text::new("BRAKE"),
                TextFont { font_size: 16.0, ..default() },
                TextColor(Color::srgb(0.3, 0.3, 0.3)),
                TeslaBrakeInd,
            ));
        });
    });

    // Circular speed gauge (bottom-centre): an arc of dots that fill with speed,
    // with a big Tesla-style digital readout in the middle.
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(10.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        HudEntity,
    )).with_children(|wrap| {
        wrap.spawn(Node { width: Val::Px(220.0), height: Val::Px(220.0), ..default() })
            .with_children(|gauge| {
                let cc = 110.0_f32; // gauge centre
                let r = 90.0_f32; // dot ring radius
                let ds = 13.0_f32; // dot size
                for i in 0..GAUGE_N {
                    // 270° arc, opening at the bottom.
                    let a = (-45.0 + (i as f32 / (GAUGE_N - 1) as f32) * 270.0).to_radians();
                    let lx = cc + r * a.cos() - ds / 2.0;
                    let ty = cc - r * a.sin() - ds / 2.0;
                    gauge.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(lx),
                            top: Val::Px(ty),
                            width: Val::Px(ds),
                            height: Val::Px(ds),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.15, 0.16, 0.2)),
                        GaugeSegment(i),
                    ));
                }
                // Centre digital readout.
                gauge
                    .spawn(Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        top: Val::Px(0.0),
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|c| {
                        c.spawn((
                            Text::new("0"),
                            TextFont { font_size: 52.0, ..default() },
                            TextColor(Color::WHITE),
                            GaugeSpeedText,
                        ));
                        c.spawn((
                            Text::new("MPH"),
                            TextFont { font_size: 18.0, ..default() },
                            TextColor(Color::srgb(0.6, 0.7, 0.85)),
                        ));
                    });
            });
    });

    // Minimap: a fixed, top-down orthographic camera framing the WHOLE track
    // (Forza style). It renders only the overlay layer (track ribbon + racer
    // dots) on a dark background — cheap and clean, not the full 3D world.
    let (center, extent) = track_bounds(&level_data.waypoints);
    commands.insert_resource(MinimapView { extent });

    commands.spawn((
        Camera3d::default(),
        Camera {
            order: 1, // Render after main camera
            viewport: Some(Viewport {
                physical_position: UVec2::new(width.saturating_sub(310), height.saturating_sub(310)),
                physical_size: UVec2::new(290, 290),
                ..default()
            }),
            clear_color: ClearColorConfig::Custom(Color::srgba(0.06, 0.07, 0.10, 0.85)),
            ..default()
        },
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::Fixed {
                width: 2.0 * extent,
                height: 2.0 * extent,
            },
            near: 0.1,
            far: 3000.0,
            ..OrthographicProjection::default_3d()
        }),
        // Straight down over the track centre; -Z is "up" on the map.
        Transform::from_xyz(center.x, 1200.0, center.z).looking_at(center, -Vec3::Z),
        RenderLayers::layer(1), // Overlay only
        MinimapCamera,
        HudEntity,
    ));

    // Track ribbon drawn onto the minimap overlay (layer 1), sized to read at the
    // zoomed-out scale. Rounded at each waypoint so corners join cleanly.
    if !level_data.waypoints.is_empty() {
        let num_wp = level_data.waypoints.len();

        let band = (extent * 0.022).max(14.0); // width of the track line
        let track_mat = materials.add(Color::srgb(0.55, 0.58, 0.65));
        let corner_mesh = meshes.add(Cylinder::new(band * 0.5, 1.0));

        for i in 0..num_wp {
            let wp1 = level_data.waypoints[i];
            let wp2 = level_data.waypoints[(i + 1) % num_wp];

            let seg_center = (wp1 + wp2) / 2.0;
            let dist = wp1.distance(wp2);
            let dir = (wp2 - wp1).normalize_or_zero();

            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(band, 1.0, dist))),
                MeshMaterial3d(track_mat.clone()),
                Transform::from_translation(seg_center + Vec3::Y * 400.0).looking_to(dir, Vec3::Y),
                RenderLayers::layer(1),
                HudEntity,
            ));
            commands.spawn((
                Mesh3d(corner_mesh.clone()),
                MeshMaterial3d(track_mat.clone()),
                Transform::from_translation(wp1 + Vec3::Y * 400.0),
                RenderLayers::layer(1),
                HudEntity,
            ));
        }

        // Gate dots around the track (yellow), with a white start/finish marker.
        let gate_mesh = meshes.add(Cylinder::new(band * 0.42, 1.0));
        let gate_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.85, 0.1),
            emissive: Color::srgb(1.0, 0.85, 0.1).to_linear(),
            unlit: true,
            ..default()
        });
        for wp in level_data.waypoints.iter() {
            commands.spawn((
                Mesh3d(gate_mesh.clone()),
                MeshMaterial3d(gate_mat.clone()),
                Transform::from_translation(*wp + Vec3::Y * 402.0),
                RenderLayers::layer(1),
                HudEntity,
            ));
        }
        commands.spawn((
            Mesh3d(meshes.add(Cylinder::new(band * 0.6, 1.0))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::WHITE,
                emissive: Color::WHITE.to_linear(),
                unlit: true,
                ..default()
            })),
            Transform::from_translation(level_data.waypoints[0] + Vec3::Y * 403.0),
            RenderLayers::layer(1),
            HudEntity,
        ));
    }
}

fn add_minimap_markers(
    mut commands: Commands,
    query: Query<(Entity, &Vehicle)>,
    marker_query: Query<&MinimapMarker>,
    view: Option<Res<MinimapView>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(view) = view else { return };
    // Dots sized to the zoomed-out view; the player is bigger and cyan so it pops.
    let ai_r = (view.extent * 0.028).max(16.0);
    let player_r = ai_r * 1.5;

    for (entity, vehicle) in query.iter() {
        let has_marker = marker_query.iter().any(|m| m.target == entity);
        if has_marker {
            continue;
        }

        let (color, radius) = if vehicle.is_player {
            (Color::srgb(0.0, 0.9, 1.0), player_r) // Cyan player
        } else {
            (Color::srgb(1.0, 0.25, 0.2), ai_r) // Red opponents
        };

        // Emissive + unlit so dots read clearly on the flat minimap.
        let mat = materials.add(StandardMaterial {
            base_color: color,
            emissive: color.to_linear(),
            unlit: true,
            ..default()
        });

        commands.spawn((
            Mesh3d(meshes.add(Sphere::new(radius))),
            MeshMaterial3d(mat),
            Transform::from_translation(Vec3::Y * 50.0),
            RenderLayers::layer(1),
            MinimapMarker { target: entity },
            HudEntity,
        ));
    }
}

fn update_minimap(
    vehicle_query: Query<&Transform, (With<Vehicle>, Without<MinimapMarker>)>,
    mut marker_query: Query<(&mut Transform, &MinimapMarker), Without<Vehicle>>,
) {
    // The minimap camera is fixed on the whole track; just move each racer's dot
    // to its car's XZ. Y = 450 keeps dots drawn on top of the track ribbon (Y 400).
    for (mut marker_transform, marker) in marker_query.iter_mut() {
        if let Ok(t) = vehicle_query.get(marker.target) {
            marker_transform.translation = Vec3::new(t.translation.x, 450.0, t.translation.z);
        }
    }
}

fn update_tesla_hud(
    player_query: Query<(&Vehicle, &bevy_rapier3d::prelude::Velocity), With<Player>>,
    mut speed_text: Query<&mut Text, With<TeslaSpeedText>>,
    mut power_bar: Query<&mut Node, (With<TeslaPowerBar>, Without<TeslaRegenBar>)>,
    mut regen_bar: Query<&mut Node, (With<TeslaRegenBar>, Without<TeslaPowerBar>)>,
    mut drift_ind: Query<&mut TextColor, (With<TeslaDriftInd>, Without<TeslaBrakeInd>)>,
    mut brake_ind: Query<&mut TextColor, (With<TeslaBrakeInd>, Without<TeslaDriftInd>)>,
) {
    if let Some((vehicle, velocity)) = player_query.iter().next() {
        if let Some(mut text) = speed_text.iter_mut().next() {
            let speed_mph = (velocity.linear.length() * 2.23694).round() as u32;
            text.0 = format!("{} MPH", speed_mph);
        }

        let throttle = vehicle.throttle.clamp(-1.0, 1.0);
        if let Some(mut p_bar) = power_bar.iter_mut().next() {
            if throttle > 0.0 && !vehicle.braking {
                p_bar.width = Val::Percent(throttle * 50.0);
            } else {
                p_bar.width = Val::Percent(0.0);
            }
        }
        if let Some(mut r_bar) = regen_bar.iter_mut().next() {
            if throttle < 0.0 || vehicle.braking {
                let amt = if vehicle.braking { 1.0 } else { -throttle };
                r_bar.width = Val::Percent(amt * 50.0);
            } else {
                r_bar.width = Val::Percent(0.0);
            }
        }

        if let Some(mut color) = drift_ind.iter_mut().next() {
            color.0 = if vehicle.drifting { Color::srgb(1.0, 0.4, 0.0) } else { Color::srgb(0.3, 0.3, 0.3) };
        }
        if let Some(mut color) = brake_ind.iter_mut().next() {
            color.0 = if vehicle.braking { Color::srgb(1.0, 0.0, 0.0) } else { Color::srgb(0.3, 0.3, 0.3) };
        }
    }
}

fn update_gauge(
    player_query: Query<&bevy_rapier3d::prelude::Velocity, With<Player>>,
    mut segments: Query<(&GaugeSegment, &mut BackgroundColor)>,
    mut speed_text: Query<&mut Text, With<GaugeSpeedText>>,
) {
    let Some(velocity) = player_query.iter().next() else { return };
    let mph = velocity.linear.length() * 2.23694;
    let frac = (mph / GAUGE_MAX_MPH).clamp(0.0, 1.0);
    let lit = (frac * GAUGE_N as f32).round() as usize;

    for (seg, mut bg) in segments.iter_mut() {
        let i = seg.0;
        *bg = if i < lit {
            // Green → yellow → red as the arc fills.
            let t = i as f32 / GAUGE_N as f32;
            if t < 0.6 {
                Color::srgb(0.0, 0.9, 1.0)
            } else if t < 0.85 {
                Color::srgb(1.0, 0.8, 0.0)
            } else {
                Color::srgb(1.0, 0.25, 0.1)
            }
            .into()
        } else {
            Color::srgb(0.15, 0.16, 0.2).into()
        };
    }

    if let Some(mut text) = speed_text.iter_mut().next() {
        text.0 = format!("{}", mph.round() as u32);
    }
}

fn update_place_and_hud(
    mut commands: Commands,
    mut hud_texts: Query<&mut Text, With<PlaceText>>,
    mut time_texts: Query<&mut Text, (With<TimeText>, Without<PlaceText>)>,
    mut trackers: Query<(Entity, &mut LapTracker, &Transform, Option<&Player>)>,
    time: Res<Time>,
    level_data: Res<LevelData>,
    mut game_state: ResMut<NextState<GameState>>,
) {
    let mut sorted_racers: Vec<(Entity, u32, usize, f32, bool, f32)> = Vec::new();
    let now = time.elapsed_secs();
    
    let mut player_place = 1;
    let mut total_racers = 0;
    let mut player_finished = false;

    // First update all lap times and collect data for sorting
    for (entity, mut tracker, transform, is_player) in trackers.iter_mut() {
        total_racers += 1;
        
        // Setup initial times if 0
        if tracker.race_start_time == 0.0 {
            tracker.race_start_time = now;
            tracker.current_lap_start_time = now;
        }

        if tracker.current_lap > tracker.total_laps && tracker.finished_time.is_none() {
            tracker.finished_time = Some(now - tracker.race_start_time);
            if is_player.is_some() {
                player_finished = true;
            }
        }

        let mut dist_to_next = 0.0;
        if !level_data.waypoints.is_empty() {
            let target = level_data.waypoints[tracker.next_waypoint];
            dist_to_next = transform.translation.distance(target);
        }

        sorted_racers.push((
            entity,
            tracker.current_lap,
            tracker.next_waypoint,
            dist_to_next,
            is_player.is_some(),
            tracker.finished_time.unwrap_or(99999.0)
        ));
    }

    // Sort racers: 
    // 1. Finished? (lower finished time is better)
    // 2. Current lap (higher is better)
    // 3. Next waypoint (higher is better)
    // 4. Distance to next waypoint (lower is better)
    sorted_racers.sort_by(|a, b| {
        if a.5 < 99999.0 || b.5 < 99999.0 {
            a.5.partial_cmp(&b.5).unwrap()
        } else if a.1 != b.1 {
            b.1.cmp(&a.1)
        } else if a.2 != b.2 {
            b.2.cmp(&a.2)
        } else {
            a.3.partial_cmp(&b.3).unwrap()
        }
    });

    // Find player place
    for (i, racer) in sorted_racers.iter().enumerate() {
        if racer.4 {
            player_place = i + 1;
            break;
        }
    }

    let suffix = match player_place {
        1 => "st",
        2 => "nd",
        3 => "rd",
        _ => "th",
    };

    for mut text in &mut hud_texts {
        text.0 = format!("Place: {}{} / {}", player_place, suffix, total_racers);
    }

    // Update time text for player
    for (_, tracker, _, is_player) in trackers.iter() {
        if is_player.is_some() {
            for mut text in &mut time_texts {
                let total_elapsed = tracker.finished_time.unwrap_or(now - tracker.race_start_time);
                let current_lap_elapsed = now - tracker.current_lap_start_time;
                
                let mins = (total_elapsed / 60.0).floor() as u32;
                let secs = (total_elapsed % 60.0).floor() as u32;
                let millis = ((total_elapsed % 1.0) * 100.0).floor() as u32;
                
                let lmins = (current_lap_elapsed / 60.0).floor() as u32;
                let lsecs = (current_lap_elapsed % 60.0).floor() as u32;
                let lmillis = ((current_lap_elapsed % 1.0) * 100.0).floor() as u32;
                
                text.0 = format!("Lap: {}/{}\nTotal Time: {:02}:{:02}.{:02}\nLap Time: {:02}:{:02}.{:02}", 
                    tracker.current_lap.min(tracker.total_laps), tracker.total_laps,
                    mins, secs, millis,
                    lmins, lsecs, lmillis
                );
            }
            break;
        }
    }

    if player_finished {
        // Snapshot the final standings NOW — the cars (and their LapTrackers) are
        // despawned on OnExit(Racing), before the scoreboard reads them.
        let rows = sorted_racers
            .iter()
            .map(|r| RaceResultRow {
                is_player: r.4,
                finished_time: if r.5 < 99999.0 { Some(r.5) } else { None },
                current_lap: r.1,
            })
            .collect();
        commands.insert_resource(RaceResults { rows });
        game_state.set(GameState::Scoreboard);
    }
}

/// "Continue" on the scoreboard routes to PostRace; from here we kick off a fresh
/// race (regenerate the level) instead of leaving the player at a dead end.
fn start_next_race(mut game_state: ResMut<NextState<GameState>>) {
    game_state.set(GameState::GeneratingLevel);
}

fn cleanup_hud(mut commands: Commands, query: Query<Entity, With<HudEntity>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}

// SCOREBOARD LOGIC

fn setup_scoreboard(
    mut commands: Commands,
    results: Option<Res<RaceResults>>,
) {
    // Standings were captured at race end (cars are already gone). Fall back to an
    // empty list if somehow missing.
    let empty = Vec::new();
    let rows: &Vec<RaceResultRow> = results.as_ref().map(|r| &r.rows).unwrap_or(&empty);

    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            flex_direction: FlexDirection::Column,
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.8)), // Transparent dark overlay
        ScoreboardEntity,
    )).with_children(|parent| {
        parent.spawn((
            Text::new("RACE RESULTS"),
            TextFont { font_size: 60.0, ..default() },
            TextColor(Color::WHITE),
            Node { margin: UiRect::all(Val::Px(20.0)), ..default() },
        ));

        let panel = Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexStart,
            padding: UiRect::all(Val::Px(20.0)),
            ..default()
        };
        
        parent.spawn((panel, BackgroundColor(Color::srgb(0.1, 0.1, 0.15)))).with_children(|list| {
            for (i, row) in rows.iter().enumerate() {
                let name = if row.is_player { "Player" } else { "AI Racer" };
                let color = if row.is_player { Color::srgb(0.0, 1.0, 0.0) } else { Color::WHITE };

                let time_str = if let Some(t) = row.finished_time {
                    let mins = (t / 60.0).floor() as u32;
                    let secs = (t % 60.0).floor() as u32;
                    let millis = ((t % 1.0) * 100.0).floor() as u32;
                    format!("{:02}:{:02}.{:02}", mins, secs, millis)
                } else {
                    format!("Lap {} (DNF)", row.current_lap)
                };

                list.spawn((
                    Text::new(format!("{}. {} - {}", i + 1, name, time_str)),
                    TextFont { font_size: 24.0, ..default() },
                    TextColor(color),
                ));
            }
        });

        // Buttons
        parent.spawn(Node {
            flex_direction: FlexDirection::Row,
            margin: UiRect::all(Val::Px(20.0)),
            ..default()
        }).with_children(|btns| {
            // Main Menu Button
            btns.spawn((
                Button,
                Node {
                    width: Val::Px(200.0),
                    height: Val::Px(50.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    margin: UiRect::all(Val::Px(10.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.3, 0.3, 0.3)),
                ScoreboardBtn::MainMenu,
            )).with_child((
                Text::new("Main Menu"),
                TextFont { font_size: 24.0, ..default() },
                TextColor(Color::WHITE),
            ));

            // Continue Button
            btns.spawn((
                Button,
                Node {
                    width: Val::Px(200.0),
                    height: Val::Px(50.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    margin: UiRect::all(Val::Px(10.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.0, 0.8, 1.0)),
                ScoreboardBtn::Continue,
            )).with_child((
                Text::new("Continue"),
                TextFont { font_size: 24.0, ..default() },
                TextColor(Color::BLACK),
            ));
        });
    });
}

fn scoreboard_interaction(
    mut interaction_query: Query<(&Interaction, &mut BackgroundColor, &ScoreboardBtn), Changed<Interaction>>,
    mut game_state: ResMut<NextState<GameState>>,
) {
    for (interaction, mut color, btn) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                match btn {
                    ScoreboardBtn::MainMenu => game_state.set(GameState::MainMenu),
                    ScoreboardBtn::Continue => game_state.set(GameState::PostRace),
                }
            }
            Interaction::Hovered => {
                match btn {
                    ScoreboardBtn::MainMenu => *color = Color::srgb(0.4, 0.4, 0.4).into(),
                    ScoreboardBtn::Continue => *color = Color::srgb(0.5, 1.0, 1.0).into(),
                }
            }
            Interaction::None => {
                match btn {
                    ScoreboardBtn::MainMenu => *color = Color::srgb(0.3, 0.3, 0.3).into(),
                    ScoreboardBtn::Continue => *color = Color::srgb(0.0, 0.8, 1.0).into(),
                }
            }
        }
    }
}

fn cleanup_scoreboard(mut commands: Commands, query: Query<Entity, With<ScoreboardEntity>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}
