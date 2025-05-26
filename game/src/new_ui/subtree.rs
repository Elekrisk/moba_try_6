use bevy::ecs::query::QueryData;
pub use bevy::prelude::*;

use super::{
    View, Widget,
    tree::{IntoUiFunc, UiFunc, UiTree},
};

pub struct SubtreeView<F: UiFunc + Clone> {
    label: String,
    subtree: F,
}

impl<F: UiFunc + Clone> SubtreeView<F> {
    pub fn new<M, I: IntoUiFunc<M, UiFunc = F>>(label: impl Into<String>, subtree: I) -> Self {
        Self {
            label: label.into(),
            subtree: subtree.into_ui_func(),
        }
    }
}

impl<F: UiFunc + Clone> View for SubtreeView<F> {
    type Widget = SubtreeWidget;

    fn build(&mut self, parent: &mut ChildSpawnerCommands) -> Self::Widget {
        let e = parent.spawn(UiTree::new(self.subtree.clone())).id();

        SubtreeWidget {
            entity: e,
            parent: parent.target_entity(),
        }
    }

    fn rebuild(&mut self, prev: &Self, widget: &mut Self::Widget, mut commands: Commands) {
        if self.label != prev.label {
            commands
                .entity(widget.entity())
                .insert(UiTree::new(self.subtree.clone()));
        }
    }
}

pub struct SubtreeWidget {
    entity: Entity,
    parent: Entity,
}

impl Widget for SubtreeWidget {
    fn entity(&self) -> Entity {
        self.entity
    }

    fn parent(&self) -> Entity {
        self.parent
    }
}
