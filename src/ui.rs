use bevy::prelude::*;
use crate::game_state::{GameState, GameDifficulty};

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::MainMenu), setup_main_menu)
           .add_systems(Update, (
               menu_interaction,
               difficulty_interaction,
               sensitivity_interaction,
               update_settings_text
           ).run_if(in_state(GameState::MainMenu)))
           .add_systems(OnExit(GameState::MainMenu), cleanup_main_menu);
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
        BackgroundColor(Color::srgb(1.0, 0.9, 0.1)),
        MainMenuEntity,
    )).with_children(|parent| {
        // Title
        parent.spawn((
            Text::new("RUST RACER"),
            TextFont {
                font_size: 80.0,
                ..default()
            },
            TextColor(Color::srgb(0.9, 0.1, 0.1)),
            Node {
                margin: UiRect::all(Val::Px(50.0)),
                ..default()
            },
        ));

        // Settings container
        parent.spawn((
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                margin: UiRect::all(Val::Px(20.0)),
                padding: UiRect::all(Val::Px(20.0)),
                border: UiRect::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(1.0, 0.8, 0.0)), // Slightly darker yellow
        )).with_children(|settings| {
            // Difficulty row
            settings.spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    margin: UiRect::all(Val::Px(10.0)),
                    ..default()
                },
            )).with_children(|row| {
                row.spawn((Text::new("Difficulty: "), TextFont { font_size: 30.0, ..default() }, TextColor(Color::BLACK), Node { width: Val::Px(180.0), ..default() }));
                
                row.spawn((Button, Node { width: Val::Px(40.0), height: Val::Px(40.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, margin: UiRect::all(Val::Px(10.0)), ..default() }, BackgroundColor(Color::srgb(0.8, 0.8, 0.8)), DifficultyBtn::Decrease))
                   .with_child((Text::new("-"), TextFont { font_size: 30.0, ..default() }, TextColor(Color::BLACK)));
                
                row.spawn((Text::new(format!("{:.1}", difficulty.ai_aggressiveness)), TextFont { font_size: 30.0, ..default() }, TextColor(Color::BLACK), Node { width: Val::Px(60.0), justify_content: JustifyContent::Center, ..default() }, DifficultyText));
                
                row.spawn((Button, Node { width: Val::Px(40.0), height: Val::Px(40.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, margin: UiRect::all(Val::Px(10.0)), ..default() }, BackgroundColor(Color::srgb(0.8, 0.8, 0.8)), DifficultyBtn::Increase))
                   .with_child((Text::new("+"), TextFont { font_size: 30.0, ..default() }, TextColor(Color::BLACK)));
            });

            // Sensitivity row
            settings.spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    margin: UiRect::all(Val::Px(10.0)),
                    ..default()
                },
            )).with_children(|row| {
                row.spawn((Text::new("Steering Sens: "), TextFont { font_size: 30.0, ..default() }, TextColor(Color::BLACK), Node { width: Val::Px(180.0), ..default() }));
                
                row.spawn((Button, Node { width: Val::Px(40.0), height: Val::Px(40.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, margin: UiRect::all(Val::Px(10.0)), ..default() }, BackgroundColor(Color::srgb(0.8, 0.8, 0.8)), SensitivityBtn::Decrease))
                   .with_child((Text::new("-"), TextFont { font_size: 30.0, ..default() }, TextColor(Color::BLACK)));
                
                row.spawn((Text::new(format!("{:.1}", difficulty.steering_sensitivity)), TextFont { font_size: 30.0, ..default() }, TextColor(Color::BLACK), Node { width: Val::Px(60.0), justify_content: JustifyContent::Center, ..default() }, SensitivityText));
                
                row.spawn((Button, Node { width: Val::Px(40.0), height: Val::Px(40.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, margin: UiRect::all(Val::Px(10.0)), ..default() }, BackgroundColor(Color::srgb(0.8, 0.8, 0.8)), SensitivityBtn::Increase))
                   .with_child((Text::new("+"), TextFont { font_size: 30.0, ..default() }, TextColor(Color::BLACK)));
            });
        });

        // Start Button
        parent.spawn((
            Button,
            Node {
                width: Val::Px(300.0),
                height: Val::Px(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                margin: UiRect::all(Val::Px(30.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.1, 0.8, 1.0)),
            StartButton,
        )).with_child((
            Text::new("START RACE"),
            TextFont { font_size: 40.0, ..default() },
            TextColor(Color::BLACK),
        ));
    });
}



fn menu_interaction(
    mut interaction_query: Query<(&Interaction, &mut BackgroundColor), (Changed<Interaction>, With<StartButton>)>,
    mut game_state: ResMut<NextState<GameState>>,
) {
    for (interaction, mut color) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => game_state.set(GameState::GeneratingLevel),
            Interaction::Hovered => *color = Color::srgb(0.3, 0.9, 1.0).into(),
            Interaction::None => *color = Color::srgb(0.1, 0.8, 1.0).into(),
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

fn update_settings_text(
    difficulty: Res<GameDifficulty>,
    mut diff_text: Query<&mut Text, (With<DifficultyText>, Without<SensitivityText>)>,
    mut sens_text: Query<&mut Text, (With<SensitivityText>, Without<DifficultyText>)>,
) {
    if difficulty.is_changed() {
        for mut text in &mut diff_text {
            text.0 = format!("{:.1}", difficulty.ai_aggressiveness);
        }
        for mut text in &mut sens_text {
            text.0 = format!("{:.1}", difficulty.steering_sensitivity);
        }
    }
}

fn cleanup_main_menu(mut commands: Commands, query: Query<Entity, With<MainMenuEntity>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}
