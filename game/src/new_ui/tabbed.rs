use bevy::prelude::*;

use super::{
    button::ButtonView, list::{ListView, ListWidget}, stylable::{Stylable, StylableWidget}, text::TextView, BoxedView, BoxedWidget, ErasedView, View, ViewExt, Widget
};

pub fn client(app: &mut App) {
    app.add_observer(on_current_tab_changed);
}

pub struct TabbedView {
    pub tabs: Stylable<ListView>,
    pub pages: Stylable<ListView>,
}

impl TabbedView {
    pub fn new() -> Self {
        Self {
            tabs: ListView::new().styled(),
            pages: ListView::new().styled(),
        }
    }

    pub fn with(mut self, title: impl Into<String>, page: impl ErasedView) -> Self {
        self.add(title, page);
        self
    }

    pub fn add(&mut self, title: impl Into<String>, page: impl ErasedView) {
        let index = self.tabs.inner.items.len();
        self.tabs.inner.add(ButtonView::new(
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
        ));
        self.pages.inner.add(page);
    }
}

#[derive(Component, Default)]
#[component(immutable)]
pub struct CurrentTab(usize);

impl View for TabbedView {
    type Widget = TabbedWidget;

    fn build(&mut self, parent: &mut ChildSpawnerCommands) -> Self::Widget {
        let mut tbw = None;
        let mut tpw = None;

        parent
            .spawn(Node {
                flex_direction: FlexDirection::Column,
                ..default()
            })
            .with_children(|parent| {
                tbw = Some(self.tabs.build(parent));
                tpw = Some(self.pages.build(parent));
            });

        TabbedWidget {
            entity: Entity::PLACEHOLDER,
            parent: parent.target_entity(),
            tab_bar_widget: tbw.unwrap(),
            tab_pages_widget: tpw.unwrap(),
        }
    }

    fn rebuild(&mut self, prev: &Self, widget: &mut Self::Widget, mut commands: Commands) {
        self.tabs.rebuild(&prev.tabs, &mut widget.tab_bar_widget, commands.reborrow());
        self.pages.rebuild(&prev.pages, &mut widget.tab_pages_widget, commands.reborrow());

        let max_index = self.tabs.inner.items.len().saturating_sub(1);

        commands.entity(widget.entity).queue(move |mut entity: EntityWorldMut<'_>| {
            if entity.get::<CurrentTab>().unwrap().0 > max_index {
                entity.insert(CurrentTab(max_index));
            }
        });
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

fn on_current_tab_changed(trigger: Trigger<OnInsert, CurrentTab>, current_tab: Query<&CurrentTab>, children: Query<&Children>, mut node: Query<&mut Node>) {
    // Set chosen page visible
    let index = current_tab.get(trigger.target()).unwrap().0;
    let tab_pages_e = children.get(trigger.target()).unwrap()[1];
    let tab_pages = children.get(tab_pages_e).unwrap();

    for (i, e) in tab_pages.iter().enumerate() {
        let mut node = node.get_mut(e).unwrap();
        if i == index {
            node.display = Display::None;
        } else {
            node.display = Display::Flex;
        }
    }
}
