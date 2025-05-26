use bevy::{
    ecs::{
        system::SystemId,
        world::{self, CommandQueue, DeferredWorld},
    },
    prelude::*,
};
use game::new_ui::{
    button::ButtonView, list::ListView, stylable::Stylable, subtree::SubtreeView, text::TextView,
    tree::UiTree, *,
};

fn main() -> AppExit {
    App::new()
        .add_plugins((
            DefaultPlugins,
            game::new_ui::client,
            game::ui::style::client,
            client,
        ))
        .insert_resource(CurrentTab(0))
        .run()
}

fn client(app: &mut App) {
    app.insert_resource(State(0.0))
        .add_systems(Startup, setup_ui);
}

fn setup_ui(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.spawn((UiTree::new::<(u32, _, _)>(ui),));
}

#[derive(Resource)]
struct State(f32);

fn ui(state: Res<State>) -> Option<Stylable<ListView>> {
    let changed = state.is_changed();
    if changed {
    } else {
        return None;
    }

    println!("Rendering!");

    let list = ListView::new()
        .with(ButtonView::new(
            TextView::new("-"),
            "state minus",
            |mut state: ResMut<State>| state.0 -= 1.0,
        ))
        .with(TextView::new(state.0.to_string()))
        .with(ButtonView::new(
            TextView::new("+"),
            "state plus",
            |mut state: ResMut<State>| state.0 += 1.0,
        ))
        .styled()
        .with("column_gap", Val::Px(10.0))
        .unwrap();

    Some(
        ListView::new()
            .with(list)
            .with(SubtreeView::new("tabs", tabs))
            .styled()
            .with("flex_direction", FlexDirection::Column)
            .unwrap(),
    )
}

#[derive(Resource)]
struct CurrentTab(usize);

fn tabs(tab: Res<CurrentTab>) -> Option<impl View + use<>> {
    if !tab.is_changed() {
        return None;
    }

    let tab_bar = ListView::new().with(
        ListView::new()
            .with(ButtonView::new(
                TextView::new("Tab 1"),
                "button.tab.1",
                set_tab(0),
            ))
            .with(ButtonView::new(
                TextView::new("Tab 2"),
                "button.tab.2",
                set_tab(1),
            ))
            .with(ButtonView::new(
                TextView::new("Tab 3"),
                "button.tab.3",
                set_tab(2),
            )),
    );

    let shown_tab = match tab.0 {
        0 => tab1().boxed(),
        1 => tab2().boxed(),
        2 => tab3().boxed(),
        _ => unreachable!(),
    };

    Some(
        ListView::new()
            .with(tab_bar)
            .with(shown_tab)
            .styled()
            .with("flex_direction", FlexDirection::Column)
            .unwrap(),
    )
}

fn set_tab(new_tab: usize) -> impl System<In = (), Out = ()> + Clone {
    IntoSystem::into_system(move |mut tab: ResMut<CurrentTab>| tab.0 = new_tab)
}

fn tab1() -> impl View {
    TextView::new("Tab 1 content")
}

fn tab2() -> impl View {
    TextView::new("Tab 2 content")
}

fn tab3() -> impl View {
    TextView::new("Tab 3 content")
}
