use bevy::{
    prelude::*,
    ui::experimental::GhostNode,
};

fn main() -> AppExit {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup_ui)
        .add_systems(Update, spawn_text_on_space)
        .run()
}

#[derive(Component)]
struct InsertHere(String);

fn setup_ui(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.spawn((
        Node {
            flex_direction: FlexDirection::Column,
            ..default()
        },
        InsertHere("Base".into()),
        children![
            (GhostNode, InsertHere("One ghost node".into())),
            (
                GhostNode,
                children![(
                    GhostNode,
                    InsertHere("Two ghost nodes".into()),
                    // Children::default() // Uncommenting this makes the children render,
                ),],
            ),
        ],
    ));
}

fn spawn_text_on_space(
    input: Res<ButtonInput<KeyCode>>,
    parents: Query<(Entity, &InsertHere)>,
    mut commands: Commands,
) {
    if input.just_pressed(KeyCode::Space) {
        for (parent, InsertHere(level)) in &parents {
            commands
                .entity(parent)
                .with_child(Text::new(format!("{level} Child",)));
        }
    }
}
