use bevy::prelude::*;
use crate::game_state::GameState;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::MainMenu), setup_main_menu)
           .add_systems(Update, menu_interaction.run_if(in_state(GameState::MainMenu)))
           .add_systems(OnExit(GameState::MainMenu), cleanup_main_menu);
    }
}

#[derive(Component)]
struct MainMenuEntity;

fn setup_main_menu(mut commands: Commands) {
    // Main Container
    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            flex_direction: FlexDirection::Column,
            ..default()
        },
        // Comic book pop-art style background: bright yellow
        BackgroundColor(Color::srgb(1.0, 0.9, 0.1)),
        MainMenuEntity,
    )).with_children(|parent| {
        // Title Text
        parent.spawn((
            Text::new("RUST RACER"),
            TextFont {
                font_size: 80.0,
                ..default()
            },
            TextColor(Color::srgb(0.9, 0.1, 0.1)), // Bold red
            Node {
                margin: UiRect::all(Val::Px(50.0)),
                ..default()
            },
        ));

        // Start Button
        parent.spawn((
            Button,
            Node {
                width: Val::Px(300.0),
                height: Val::Px(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgb(0.1, 0.8, 1.0)), // Cyan button
        )).with_child((
            Text::new("START RACE"),
            TextFont {
                font_size: 40.0,
                ..default()
            },
            TextColor(Color::BLACK),
        ));
    });
}

fn menu_interaction(
    mut interaction_query: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
    mut game_state: ResMut<NextState<GameState>>,
) {
    for (interaction, mut color) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                game_state.set(GameState::GeneratingLevel);
            }
            Interaction::Hovered => {
                // Brighter cyan when hovered
                *color = Color::srgb(0.3, 0.9, 1.0).into();
            }
            Interaction::None => {
                // Normal cyan
                *color = Color::srgb(0.1, 0.8, 1.0).into();
            }
        }
    }
}

fn cleanup_main_menu(
    mut commands: Commands,
    query: Query<Entity, With<MainMenuEntity>>,
) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}
