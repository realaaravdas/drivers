use bevy::prelude::*;
use crate::game_state::{GameState, GameDifficulty};

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Splash), setup_splash_screen)
           .add_systems(Update, splash_timer.run_if(in_state(GameState::Splash)))
           .add_systems(OnExit(GameState::Splash), cleanup_splash_screen)
           .add_systems(OnEnter(GameState::MainMenu), setup_main_menu)
           .add_systems(Update, (
               menu_interaction,
               difficulty_interaction,
               sensitivity_interaction,
               speed_interaction,
               accel_interaction,
               laps_interaction,
               update_settings_text
           ).run_if(in_state(GameState::MainMenu)))
           .add_systems(OnExit(GameState::MainMenu), cleanup_main_menu)
           .add_systems(OnEnter(GameState::Loading), setup_loading_screen)
           .add_systems(Update, loading_timer.run_if(in_state(GameState::Loading)))
           .add_systems(OnExit(GameState::Loading), cleanup_loading_screen);
    }
}

#[derive(Component)]
struct MainMenuEntity;

#[derive(Component)]
struct StartButton;

#[derive(Component)]
enum DifficultyBtn { Decrease, Increase }

#[derive(Component)]
struct DifficultyText;

#[derive(Component)]
enum SensitivityBtn { Decrease, Increase }

#[derive(Component)]
struct SensitivityText;

#[derive(Component)]
enum SpeedBtn { Decrease, Increase }

#[derive(Component)]
struct SpeedText;

#[derive(Component)]
enum AccelBtn { Decrease, Increase }

#[derive(Component)]
struct AccelText;

#[derive(Component)]
enum LapsBtn { Decrease, Increase }

#[derive(Component)]
struct LapsText;

fn setup_main_menu(mut commands: Commands, difficulty: Res<GameDifficulty>) {
    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            flex_direction: FlexDirection::Column,
            ..default()
        },
        BackgroundColor(Color::srgb(0.05, 0.05, 0.08)), // Dark modern background
        MainMenuEntity,
    )).with_children(|parent| {
        // Title
        parent.spawn((
            Text::new("RUST RACER"),
            TextFont {
                font_size: 90.0,
                ..default()
            },
            TextColor(Color::srgb(0.0, 0.8, 1.0)), // Cyan neon title
            Node {
                margin: UiRect::all(Val::Px(40.0)),
                ..default()
            },
        ));

        // Settings container
        parent.spawn((
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                margin: UiRect::all(Val::Px(20.0)),
                padding: UiRect::all(Val::Px(30.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.1, 0.1, 0.15)), // Dark panel
        )).with_children(|settings| {
            
            let row_node = Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                width: Val::Px(400.0),
                margin: UiRect::all(Val::Px(10.0)),
                ..default()
            };
            let label_node = Node { width: Val::Px(200.0), ..default() };
            let val_node = Node { width: Val::Px(80.0), justify_content: JustifyContent::Center, ..default() };
            let btn_node = Node { width: Val::Px(40.0), height: Val::Px(40.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() };
            let btn_color = BackgroundColor(Color::srgb(0.2, 0.2, 0.3));
            let text_font = TextFont { font_size: 24.0, ..default() };
            let text_color = TextColor(Color::WHITE);

            // Difficulty row
            settings.spawn(row_node.clone()).with_children(|row| {
                row.spawn((Text::new("AI Aggression"), text_font.clone(), text_color, label_node.clone()));
                row.spawn((Button, btn_node.clone(), btn_color, DifficultyBtn::Decrease)).with_child((Text::new("-"), text_font.clone(), text_color));
                row.spawn((Text::new(format!("{:.1}", difficulty.ai_aggressiveness)), text_font.clone(), text_color, val_node.clone(), DifficultyText));
                row.spawn((Button, btn_node.clone(), btn_color, DifficultyBtn::Increase)).with_child((Text::new("+"), text_font.clone(), text_color));
            });

            // Sensitivity row
            settings.spawn(row_node.clone()).with_children(|row| {
                row.spawn((Text::new("Steering Sens"), text_font.clone(), text_color, label_node.clone()));
                row.spawn((Button, btn_node.clone(), btn_color, SensitivityBtn::Decrease)).with_child((Text::new("-"), text_font.clone(), text_color));
                row.spawn((Text::new(format!("{:.1}", difficulty.steering_sensitivity)), text_font.clone(), text_color, val_node.clone(), SensitivityText));
                row.spawn((Button, btn_node.clone(), btn_color, SensitivityBtn::Increase)).with_child((Text::new("+"), text_font.clone(), text_color));
            });

            // Top Speed row
            settings.spawn(row_node.clone()).with_children(|row| {
                row.spawn((Text::new("Top Speed"), text_font.clone(), text_color, label_node.clone()));
                row.spawn((Button, btn_node.clone(), btn_color, SpeedBtn::Decrease)).with_child((Text::new("-"), text_font.clone(), text_color));
                row.spawn((Text::new(format!("{:.0}", difficulty.top_speed)), text_font.clone(), text_color, val_node.clone(), SpeedText));
                row.spawn((Button, btn_node.clone(), btn_color, SpeedBtn::Increase)).with_child((Text::new("+"), text_font.clone(), text_color));
            });

            // Acceleration row
            settings.spawn(row_node.clone()).with_children(|row| {
                row.spawn((Text::new("Acceleration"), text_font.clone(), text_color, label_node.clone()));
                row.spawn((Button, btn_node.clone(), btn_color, AccelBtn::Decrease)).with_child((Text::new("-"), text_font.clone(), text_color));
                row.spawn((Text::new(format!("{:.0}", difficulty.acceleration)), text_font.clone(), text_color, val_node.clone(), AccelText));
                row.spawn((Button, btn_node.clone(), btn_color, AccelBtn::Increase)).with_child((Text::new("+"), text_font.clone(), text_color));
            });

            // Laps row
            settings.spawn(row_node.clone()).with_children(|row| {
                row.spawn((Text::new("Laps"), text_font.clone(), text_color, label_node.clone()));
                row.spawn((Button, btn_node.clone(), btn_color, LapsBtn::Decrease)).with_child((Text::new("-"), text_font.clone(), text_color));
                row.spawn((Text::new(format!("{}", difficulty.laps)), text_font.clone(), text_color, val_node.clone(), LapsText));
                row.spawn((Button, btn_node.clone(), btn_color, LapsBtn::Increase)).with_child((Text::new("+"), text_font.clone(), text_color));
            });
        });

        // Start Button
        parent.spawn((
            Button,
            Node {
                width: Val::Px(300.0),
                height: Val::Px(80.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                margin: UiRect::all(Val::Px(40.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.0, 0.8, 1.0)),
            StartButton,
        )).with_child((
            Text::new("START RACE"),
            TextFont { font_size: 32.0, ..default() },
            TextColor(Color::srgb(0.05, 0.05, 0.08)), // Dark text on bright button
        ));
    });
}



fn menu_interaction(
    mut interaction_query: Query<(&Interaction, &mut BackgroundColor), (Changed<Interaction>, With<StartButton>)>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    for (interaction, mut color) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                *color = Color::srgb(0.0, 0.4, 0.0).into();
                next_state.set(GameState::Loading);
            }
            Interaction::Hovered => *color = Color::srgb(0.5, 1.0, 1.0).into(),
            Interaction::None => *color = Color::srgb(0.0, 0.8, 1.0).into(),
        }
    }
}

fn difficulty_interaction(
    interaction_query: Query<(&Interaction, &DifficultyBtn), Changed<Interaction>>,
    mut difficulty: ResMut<GameDifficulty>,
) {
    for (interaction, btn) in &interaction_query {
        if *interaction == Interaction::Pressed {
            match btn {
                DifficultyBtn::Decrease => difficulty.ai_aggressiveness = (difficulty.ai_aggressiveness - 0.2).max(0.2),
                DifficultyBtn::Increase => difficulty.ai_aggressiveness = (difficulty.ai_aggressiveness + 0.2).min(3.0),
            }
        }
    }
}

fn sensitivity_interaction(
    interaction_query: Query<(&Interaction, &SensitivityBtn), Changed<Interaction>>,
    mut difficulty: ResMut<GameDifficulty>,
) {
    for (interaction, btn) in &interaction_query {
        if *interaction == Interaction::Pressed {
            match btn {
                SensitivityBtn::Decrease => difficulty.steering_sensitivity = (difficulty.steering_sensitivity - 0.5).max(0.5),
                SensitivityBtn::Increase => difficulty.steering_sensitivity = (difficulty.steering_sensitivity + 0.5).min(10.0),
            }
        }
    }
}

fn speed_interaction(
    interaction_query: Query<(&Interaction, &SpeedBtn), Changed<Interaction>>,
    mut difficulty: ResMut<GameDifficulty>,
) {
    for (interaction, btn) in &interaction_query {
        if *interaction == Interaction::Pressed {
            match btn {
                SpeedBtn::Decrease => difficulty.top_speed = (difficulty.top_speed - 10.0).max(40.0),
                SpeedBtn::Increase => difficulty.top_speed = (difficulty.top_speed + 10.0).min(300.0),
            }
        }
    }
}

fn accel_interaction(
    interaction_query: Query<(&Interaction, &AccelBtn), Changed<Interaction>>,
    mut difficulty: ResMut<GameDifficulty>,
) {
    for (interaction, btn) in &interaction_query {
        if *interaction == Interaction::Pressed {
            match btn {
                AccelBtn::Decrease => difficulty.acceleration = (difficulty.acceleration - 50.0).max(50.0),
                AccelBtn::Increase => difficulty.acceleration = (difficulty.acceleration + 50.0).min(2000.0),
            }
        }
    }
}

fn laps_interaction(
    interaction_query: Query<(&Interaction, &LapsBtn), Changed<Interaction>>,
    mut difficulty: ResMut<GameDifficulty>,
) {
    for (interaction, btn) in &interaction_query {
        if *interaction == Interaction::Pressed {
            match btn {
                LapsBtn::Decrease => difficulty.laps = (difficulty.laps.saturating_sub(1)).max(1),
                LapsBtn::Increase => difficulty.laps = (difficulty.laps + 1).min(10),
            }
        }
    }
}

fn update_settings_text(
    difficulty: Res<GameDifficulty>,
    mut diff_text: Query<&mut Text, (With<DifficultyText>, Without<SensitivityText>, Without<SpeedText>, Without<AccelText>, Without<LapsText>)>,
    mut sens_text: Query<&mut Text, (With<SensitivityText>, Without<DifficultyText>, Without<SpeedText>, Without<AccelText>, Without<LapsText>)>,
    mut speed_text: Query<&mut Text, (With<SpeedText>, Without<DifficultyText>, Without<SensitivityText>, Without<AccelText>, Without<LapsText>)>,
    mut accel_text: Query<&mut Text, (With<AccelText>, Without<DifficultyText>, Without<SensitivityText>, Without<SpeedText>, Without<LapsText>)>,
    mut laps_text: Query<&mut Text, (With<LapsText>, Without<DifficultyText>, Without<SensitivityText>, Without<SpeedText>, Without<AccelText>)>,
) {
    if difficulty.is_changed() {
        for mut text in &mut diff_text {
            text.0 = format!("{:.1}", difficulty.ai_aggressiveness);
        }
        for mut text in &mut sens_text {
            text.0 = format!("{:.1}", difficulty.steering_sensitivity);
        }
        for mut text in &mut speed_text {
            text.0 = format!("{:.0}", difficulty.top_speed);
        }
        for mut text in &mut accel_text {
            text.0 = format!("{:.0}", difficulty.acceleration);
        }
        for mut text in &mut laps_text {
            text.0 = format!("{}", difficulty.laps);
        }
    }
}

fn cleanup_main_menu(mut commands: Commands, query: Query<Entity, With<MainMenuEntity>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}

#[derive(Component)]
struct SplashEntity;

#[derive(Resource)]
struct SplashTimer(Timer);

fn setup_splash_screen(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(SplashTimer(Timer::from_seconds(3.0, TimerMode::Once)));
    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        SplashEntity,
    )).with_children(|parent| {
        parent.spawn((
            ImageNode {
                image: asset_server.load("splash.jpg"),
                ..default()
            },
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
        ));
    });
}

fn splash_timer(
    time: Res<Time>,
    mut timer: ResMut<SplashTimer>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if timer.0.tick(time.delta()).just_finished() {
        next_state.set(GameState::MainMenu);
    }
}

fn cleanup_splash_screen(mut commands: Commands, query: Query<Entity, With<SplashEntity>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}

#[derive(Component)]
struct LoadingEntity;

#[derive(Resource)]
struct LoadingTimer(Timer);

fn setup_loading_screen(mut commands: Commands, asset_server: Res<AssetServer>) {
    // Increase to 0.5s to ensure Bevy has time to render the loading screen before blocking the thread
    commands.insert_resource(LoadingTimer(Timer::from_seconds(0.5, TimerMode::Once)));
    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            flex_direction: FlexDirection::Column,
            ..default()
        },
        BackgroundColor(Color::srgb(0.05, 0.05, 0.08)),
        LoadingEntity,
    )).with_children(|parent| {
        parent.spawn((
            Text::new("GENERATING RACETRACK..."),
            TextFont {
                font_size: 60.0,
                ..default()
            },
            TextColor(Color::srgb(0.0, 0.8, 1.0)),
        ));
        parent.spawn((
            ImageNode {
                image: asset_server.load("loading.jpg"),
                ..default()
            },
            Node {
                width: Val::Px(800.0),
                height: Val::Px(450.0),
                margin: UiRect::all(Val::Px(40.0)),
                ..default()
            },
        ));
    });
}

fn loading_timer(
    time: Res<Time>,
    mut timer: ResMut<LoadingTimer>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if timer.0.tick(time.delta()).just_finished() {
        next_state.set(GameState::GeneratingLevel);
    }
}

fn cleanup_loading_screen(mut commands: Commands, query: Query<Entity, With<LoadingEntity>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}
