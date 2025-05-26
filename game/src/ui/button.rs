use bevy::{
    ecs::system::{IntoObserverSystem, ObserverSystem, SystemId},
    prelude::*,
};
use lightyear::connection::id;

use super::{
    ObservedBy,
    style::{ComponentEquals, ConditionalStyle, DefaultStyle, StyleRef, Styles, style_label},
    text::text,
};

pub fn client(app: &mut App) {
    app.add_systems(Startup, setup_button_styling);
}

pub const DEFAULT_NORMAL_COLOR: Color = Color::srgb(0.3, 0.3, 0.3);
pub const DEFAULT_HOVER_COLOR: Color = Color::srgb(0.4, 0.4, 0.4);
pub const DEFAULT_PRESS_COLOR: Color = Color::srgb(0.2, 0.2, 0.2);

style_label!(NormalButton);
style_label!(HoveredButton);
style_label!(PressedButton);

fn setup_button_styling(mut styles: ResMut<Styles>) {
    let style = styles.new_style_from(DefaultStyle, NormalButton).unwrap();
    style.background_color = Some(DEFAULT_NORMAL_COLOR);

    style.set_node_field("padding", UiRect::all(Val::Px(2.0)));
    style.set_node_field("border", UiRect::all(Val::Px(1.0)));
    style.set_node_field("margin", UiRect::all(Val::Px(2.0)));
    style.border_color = Some(Color::WHITE);

    let style = styles.new_style_from(NormalButton, HoveredButton).unwrap();
    style.background_color = Some(DEFAULT_HOVER_COLOR);
    let style = styles.new_style_from(NormalButton, PressedButton).unwrap();
    style.background_color = Some(DEFAULT_PRESS_COLOR);
}

#[derive(Component)]
pub struct ButtonStyling {
    pub normal_color: Color,
    pub hover_color: Color,
    pub press_color: Color,
    pub border: bool,
}

impl ButtonStyling {
    pub const DEFAULT: Self = Self {
        normal_color: DEFAULT_NORMAL_COLOR,
        hover_color: DEFAULT_HOVER_COLOR,
        press_color: DEFAULT_PRESS_COLOR,
        border: true,
    };
}

impl Default for ButtonStyling {
    fn default() -> Self {
        Self::DEFAULT
    }
}

pub fn button2<M: Send + Sync + 'static>(
    txt: impl Into<String>,
    on_click: impl IntoSystem<(), (), M> + Send + Sync + 'static,
) -> impl Bundle {
    let mut on_click = Some(on_click);
    (
        Button,
        // ButtonStyling::default(),
        ObservedBy::new(
            move |mut trigger: Trigger<Pointer<Click>>,
                  mut id: Local<Option<SystemId>>,
                  mut commands: Commands| {
                let id = match *id {
                    Some(id) => id,
                    None => {
                        let new_id = commands.register_system(on_click.take().unwrap());
                        *id = Some(new_id);
                        new_id
                    }
                };
                trigger.propagate(false);
                commands.run_system(id);
            },
        ),
        ConditionalStyle::new()
            .when(ComponentEquals(Interaction::None), NormalButton)
            .when(ComponentEquals(Interaction::Hovered), HoveredButton)
            .when(ComponentEquals(Interaction::Pressed), PressedButton),
        Children::spawn_one(text(txt)),
    )
}
