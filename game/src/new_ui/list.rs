use bevy::prelude::*;

use super::{BoxedView, BoxedWidget, ErasedView, View, Widget};

#[derive(Debug)]
pub struct ListView {
    pub items: Vec<BoxedView>,
}

impl ListView {
    pub fn new() -> Self {
        Self { items: default() }
    }

    pub fn from_iter<V: ErasedView>(iter: impl IntoIterator<Item = V>) -> Self {
        Self {
            items: iter.into_iter().map(BoxedView::new).collect(),
        }
    }

    pub fn with(mut self, item: impl ErasedView) -> Self {
        self.add(item);
        self
    }

    pub fn add(&mut self, item: impl ErasedView) {
        self.items.push(BoxedView {
            inner: Box::new(item),
        });
    }
}

impl View for ListView {
    type Widget = ListWidget;

    fn build(&mut self, parent: &mut ChildSpawnerCommands) -> Self::Widget {
        let mut items = vec![];
        let id = parent
            .spawn((Node { ..default() }, Name::new(self.name())))
            .with_children(|parent| {
                for item in &mut self.items {
                    items.push(item.build(parent));
                }
            })
            .id();

        ListWidget {
            entity: id,
            parent: parent.target_entity(),
            items,
        }
    }

    fn rebuild(&mut self, prev: &Self, widget: &mut Self::Widget, mut commands: Commands) {
        for ((item, prev), widget) in self
            .items
            .iter_mut()
            .zip(&prev.items)
            .zip(&mut widget.items)
        {
            item.rebuild(prev, widget, commands.reborrow());
        }

        let mut entity = commands.entity(widget.entity);
        entity.insert(Name::new(self.name()));

        if self.items.len() > prev.items.len() {
            entity.with_children(|parent| {
                for item in self.items.iter_mut().skip(prev.items.len()) {
                    widget.items.push(item.build(parent));
                }
            });
        } else if self.items.len() < prev.items.len() {
            for item in widget.items.iter().skip(self.items.len()).rev() {
                item.despawn(entity.commands());
            }
            widget.items.truncate(self.items.len());
        }
    }

    fn name(&self) -> String {
        format!("List[{}]", self.items.len())
    }
}

pub struct ListWidget {
    entity: Entity,
    parent: Entity,
    items: Vec<BoxedWidget>,
}

impl Widget for ListWidget {
    fn entity(&self) -> Entity {
        self.entity
    }
    fn parent(&self) -> Entity {
        self.parent
    }
}
