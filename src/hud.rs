use bevy::prelude::*;
use bevy_camera::Viewport;
use bevy_camera::visibility::RenderLayers;
use crate::game_state::{GameState, LapTracker};
use crate::vehicle::{Vehicle, Player};
use crate::level_gen::LevelData;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Racing), setup_hud)
           .add_systems(Update, (
               update_place_and_hud,
               add_minimap_markers,
               update_minimap,
           ).run_if(in_state(GameState::Racing)))
           .add_systems(OnExit(GameState::Racing), cleanup_hud)
           .add_systems(OnEnter(GameState::Scoreboard), setup_scoreboard)
           .add_systems(Update, scoreboard_interaction.run_if(in_state(GameState::Scoreboard)))
           .add_systems(OnExit(GameState::Scoreboard), cleanup_scoreboard);
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

    // Minimap Camera (Orthographic, looking down)
    // We use a viewport in the top-right corner
    commands.spawn((
        Camera3d::default(),
        Camera {
            order: 1, // Render after main camera
            viewport: Some(Viewport {
                physical_position: UVec2::new(width.saturating_sub(300), height.saturating_sub(300)), // Bottom right
                physical_size: UVec2::new(280, 280),
                ..default()
            }),
            clear_color: ClearColorConfig::Custom(Color::srgb(0.05, 0.05, 0.05)),
            ..default()
        },
        Projection::Perspective(PerspectiveProjection {
            fov: std::f32::consts::PI / 4.0,
            ..default()
        }),
        Transform::from_xyz(0.0, 500.0, 0.0).looking_at(Vec3::ZERO, -Vec3::Z),
        RenderLayers::from_layers(&[0, 1]), // See world AND minimap overlay
        MinimapCamera,
        HudEntity,
    ));

    // Minimap Track Outline
    // Spawn simple path high up, only visible to Minimap Camera
    if !level_data.waypoints.is_empty() {
        let num_wp = level_data.waypoints.len();
        
        let track_mat = materials.add(Color::srgba(0.0, 1.0, 0.0, 0.5));
        let corner_mesh = meshes.add(Cylinder::new(4.0, 1.0));
        
        for i in 0..num_wp {
            let wp1 = level_data.waypoints[i];
            let wp2 = level_data.waypoints[(i + 1) % num_wp];
            
            let center = (wp1 + wp2) / 2.0;
            let dist = wp1.distance(wp2);
            let dir = (wp2 - wp1).normalize_or_zero();
            
            // Path segment
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(8.0, 1.0, dist))),
                MeshMaterial3d(track_mat.clone()),
                Transform::from_translation(center + Vec3::Y * 400.0).looking_to(dir, Vec3::Y),
                RenderLayers::layer(1), // Only Minimap sees this
                HudEntity,
            ));
            
            // Smooth corner
            commands.spawn((
                Mesh3d(corner_mesh.clone()),
                MeshMaterial3d(track_mat.clone()),
                Transform::from_translation(wp1 + Vec3::Y * 400.0),
                RenderLayers::layer(1),
                HudEntity,
            ));
        }
    }
}

fn add_minimap_markers(
    mut commands: Commands,
    query: Query<(Entity, &Vehicle)>,
    marker_query: Query<&MinimapMarker>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (entity, vehicle) in query.iter() {
        let has_marker = marker_query.iter().any(|m| m.target == entity);

        if !has_marker {
            let color = if vehicle.is_player {
                Color::srgb(1.0, 0.0, 0.0) // Red for player
            } else {
                Color::srgb(0.2, 0.2, 1.0) // Blue for AI
            };

            commands.spawn((
                Mesh3d(meshes.add(Sphere::new(6.0))), // Large sphere to be visible on minimap
                MeshMaterial3d(materials.add(color)),
                Transform::from_translation(Vec3::Y * 50.0), // Start with a default height
                RenderLayers::layer(1), // Minimap layer only
                MinimapMarker { target: entity },
                HudEntity,
            ));
        }
    }
}

fn update_minimap(
    player_query: Query<&Transform, (With<Player>, Without<MinimapCamera>, Without<MinimapMarker>)>,
    mut camera_query: Query<&mut Transform, With<MinimapCamera>>,
    mut marker_query: Query<(&mut Transform, &MinimapMarker), (Without<Player>, Without<MinimapCamera>)>,
    vehicle_query: Query<&Transform, (With<Vehicle>, Without<Player>, Without<MinimapCamera>, Without<MinimapMarker>)>,
) {
    // Update camera to follow player
    if let Some(player_transform) = player_query.iter().next() {
        if let Some(mut cam_transform) = camera_query.iter_mut().next() {
            let fwd = player_transform.forward();
            let up = Vec3::Y;
            let target_pos = player_transform.translation;
            let cam_pos = target_pos - fwd * 150.0 + up * 150.0;
            
            // Lerp camera for smooth tracking
            cam_transform.translation = cam_transform.translation.lerp(cam_pos, 0.1);
            
            // Look slightly ahead of player
            let look_target = target_pos + fwd * 50.0;
            *cam_transform = cam_transform.looking_at(look_target, up);
        }

        // Update markers
        for (mut marker_transform, marker) in marker_query.iter_mut() {
            let mut target_pos = None;
            if let Ok(p_transform) = player_query.get(marker.target) {
                target_pos = Some(p_transform.translation);
            } else if let Ok(v_transform) = vehicle_query.get(marker.target) {
                target_pos = Some(v_transform.translation);
            }

            if let Some(pos) = target_pos {
                // Keep marker floating above the target
                marker_transform.translation = pos + Vec3::Y * 50.0;
            }
        }
    }
}

fn update_place_and_hud(
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
        game_state.set(GameState::Scoreboard);
    }
}

fn cleanup_hud(mut commands: Commands, query: Query<Entity, With<HudEntity>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}

// SCOREBOARD LOGIC

fn setup_scoreboard(
    mut commands: Commands,
    trackers: Query<(&LapTracker, Option<&Player>)>,
) {
    let mut sorted_racers: Vec<(&LapTracker, bool)> = trackers.iter().map(|(t, p)| (t, p.is_some())).collect();
    
    // Sort by finish time or lap/waypoint
    sorted_racers.sort_by(|(a, _), (b, _)| {
        let a_time = a.finished_time.unwrap_or(99999.0);
        let b_time = b.finished_time.unwrap_or(99999.0);
        if a_time < 99999.0 || b_time < 99999.0 {
            a_time.partial_cmp(&b_time).unwrap()
        } else if a.current_lap != b.current_lap {
            b.current_lap.cmp(&a.current_lap)
        } else {
            b.next_waypoint.cmp(&a.next_waypoint)
        }
    });

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
            for (i, (tracker, is_player)) in sorted_racers.iter().enumerate() {
                let name = if *is_player { "Player" } else { "AI Racer" };
                let color = if *is_player { Color::srgb(0.0, 1.0, 0.0) } else { Color::WHITE };
                
                let time_str = if let Some(t) = tracker.finished_time {
                    let mins = (t / 60.0).floor() as u32;
                    let secs = (t % 60.0).floor() as u32;
                    let millis = ((t % 1.0) * 100.0).floor() as u32;
                    format!("{:02}:{:02}.{:02}", mins, secs, millis)
                } else {
                    format!("Lap {}", tracker.current_lap)
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
