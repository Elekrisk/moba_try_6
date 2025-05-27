use bevy::prelude::*;

use crate::ui::style::{
    ComponentEquals, ConditionalStyle, DefaultStyle, HasComponent, Styles, style_label,
};

use super::{
    BoxedView, BoxedWidget, ErasedView, View, ViewExt, Widget,
    button::ButtonView,
    list::{ListView, ListWidget},
    stylable::{Stylable, StylableWidget},
    text::TextView,
};

pub fn client(app: &mut App) {
    app.add_observer(on_current_tab_changed)
        .add_systems(Startup, setup_styles);
}

style_label!(DefaultTabStyle);
style_label!(UnfocusTabStyle);
style_label!(HoverTabStyle);
style_label!(PressTabStyle);

fn setup_styles(mut styles: ResMut<Styles>) {
    let style = styles
        .new_style_from(DefaultStyle, DefaultTabStyle)
        .unwrap();
    style.background_color = Some(Color::srgb(0.3, 0.3, 0.3));
    let style = styles
        .new_style_from(DefaultTabStyle, UnfocusTabStyle)
        .unwrap();
    style.background_color = Some(Color::srgb(0.2, 0.2, 0.2));
    let style = styles
        .new_style_from(DefaultTabStyle, HoverTabStyle)
        .unwrap();
    style.background_color = Some(Color::srgb(0.4, 0.4, 0.4));
    let style = styles
        .new_style_from(DefaultTabStyle, PressTabStyle)
        .unwrap();
    style.background_color = Some(Color::srgb(0.2, 0.2, 0.2));
}

#[derive(Debug)]
pub struct TabbedView {
    pub tabs: Stylable<ListView>,
    pub pages: Stylable<ListView>,
}

impl TabbedView {
    pub fn new() -> Self {
        Self {
            tabs: ListView::new()
                .styled()
                .with("background_color", Color::srgb(0.2, 0.2, 0.2))
                .unwrap(),
            pages: ListView::new()
                .styled()
                .with("background_color", Color::srgb(0.3, 0.3, 0.3))
                .unwrap()
                .flex_direction(FlexDirection::Column)
                .flex_grow(1.0)
                .flex_basis(Val::Px(0.0))
                .height(Val::Px(0.0)),
        }
    }

    pub fn with(mut self, title: impl Into<String>, page: impl ErasedView) -> Self {
        self.add(title, page);
        self
    }

    pub fn add(&mut self, title: impl Into<String>, page: impl ErasedView) {
        let index = self.tabs.inner.items.len();
        let mut button = ButtonView::new(
            TextView::new(title),
            format!("tab_bar_button"),
            move |trigger: Trigger<Pointer<Click>>,
                  parent: Query<&ChildOf>,
                  mut commands: Commands| {
                // Get parent of tab button; will be tab list
                let tab_list_e = parent.get(trigger.target()).unwrap().parent();
                // Get parent of tab list; will be tabbed view
                let tab_view_e = parent.get(tab_list_e).unwrap().parent();
                // Update current tab
                commands.entity(tab_view_e).insert(CurrentTab(index));
            },
        )
        .styled();
        button.conditional = ConditionalStyle::new()
            .when(
                (
                    ComponentEquals(Interaction::None),
                    HasComponent::<FocusedTab>::new(),
                ),
                DefaultTabStyle,
            )
            .when(ComponentEquals(Interaction::None), UnfocusTabStyle)
            .when(ComponentEquals(Interaction::Hovered), HoverTabStyle)
            .when(ComponentEquals(Interaction::Pressed), PressTabStyle);
        self.tabs.inner.add(button);
        self.pages.inner.add(page);
    }
}

#[derive(Component, Default)]
#[component(immutable)]
pub struct CurrentTab(usize);

#[derive(Component, Default)]
pub struct FocusedTab;

impl View for TabbedView {
    type Widget = TabbedWidget;

    fn build(&mut self, parent: &mut ChildSpawnerCommands) -> Self::Widget {
        let mut tbw = None;
        let mut tpw = None;

        let entity = parent
            .spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    flex_grow: 1.0,
                    ..default()
                },
                Name::new("Tabbed"),
            ))
            .with_children(|parent| {
                tbw = Some(self.tabs.build(parent));
                tpw = Some(self.pages.build(parent));
            })
            .insert(CurrentTab(0))
            .id();

        TabbedWidget {
            entity,
            parent: parent.target_entity(),
            tab_bar_widget: tbw.unwrap(),
            tab_pages_widget: tpw.unwrap(),
        }
    }

    fn rebuild(&mut self, prev: &Self, widget: &mut Self::Widget, mut commands: Commands) {
        self.tabs
            .rebuild(&prev.tabs, &mut widget.tab_bar_widget, commands.reborrow());
        self.pages.rebuild(
            &prev.pages,
            &mut widget.tab_pages_widget,
            commands.reborrow(),
        );

        let max_index = self.tabs.inner.items.len().saturating_sub(1);

        commands
            .entity(widget.entity())
            .queue(move |mut entity: EntityWorldMut<'_>| {
                if entity.get::<CurrentTab>().unwrap().0 > max_index {
                    entity.insert(CurrentTab(max_index));
                }
            });
    }

    fn name(&self) -> String {
        format!("Tabbed[{}]", self.tabs.inner.items.len())
    }
}

pub struct TabbedWidget {
    entity: Entity,
    parent: Entity,
    tab_bar_widget: StylableWidget<ListWidget>,
    tab_pages_widget: StylableWidget<ListWidget>,
}
impl Widget for TabbedWidget {
    fn entity(&self) -> Entity {
        self.entity
    }

    fn parent(&self) -> Entity {
        self.parent
    }
}

fn on_current_tab_changed(
    trigger: Trigger<OnInsert, CurrentTab>,
    current_tab: Query<&CurrentTab>,
    children: Query<&Children>,
    mut node: Query<&mut Node>,
    mut commands: Commands,
) {
    // Set chosen page visible
    let index = current_tab.get(trigger.target()).unwrap().0;
    let tab_parts = children.get(trigger.target()).unwrap();
    let tab_buttons_e = tab_parts[0];
    let tab_pages_e = tab_parts[1];
    let tab_buttons = children.get(tab_buttons_e).unwrap();
    let tab_pages = children.get(tab_pages_e).unwrap();

    for ((i, e), tb) in tab_pages.iter().enumerate().zip(tab_buttons) {
        let mut node = node.get_mut(e).unwrap();
        if i == index {
            node.display = Display::Flex;
            commands.entity(*tb).insert(FocusedTab);
        } else {
            node.display = Display::None;
            commands.entity(*tb).remove::<FocusedTab>();
        }
    }
}
