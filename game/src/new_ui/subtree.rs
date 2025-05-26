use bevy::ecs::query::QueryData;
pub use bevy::prelude::*;

use super::{
    View, Widget,
    tree::{IfRunner, IntoUiFunc, OnceRunner, UiFunc, UiTree},
};

pub struct SubtreeView<F: UiFunc> {
    label: String,
    subtree: Option<F>,
}

impl<F: UiFunc> SubtreeView<F> {
    pub fn new<M, I: IntoUiFunc<M, UiFunc = F>>(label: impl Into<String>, subtree: I) -> Self {
        Self {
            label: label.into(),
            subtree: Some(subtree.into_ui_func()),
        }
    }
}

impl<F: UiFunc> SubtreeView<OnceRunner<F>> {
    pub fn once<M, I: IntoUiFunc<M, UiFunc = F>>(label: impl Into<String>, subtree: I) -> Self {
        Self {
            label: label.into(),
            subtree: Some(OnceRunner::new(subtree.into_ui_func())),
        }
    }
}

impl<F: UiFunc, C: Fn(&World) -> bool + Send + Sync + 'static> SubtreeView<IfRunner<F, C>> {
    pub fn run_if<M, I: IntoUiFunc<M, UiFunc = F>>(
        label: impl Into<String>,
        subtree: I,
        cond: C,
    ) -> Self {
        Self {
            label: label.into(),
            subtree: Some(IfRunner::new(subtree.into_ui_func(), cond)),
        }
    }
}

impl<F: UiFunc> View for SubtreeView<F> {
    type Widget = SubtreeWidget;

    fn build(&mut self, parent: &mut ChildSpawnerCommands) -> Self::Widget {
        let e = parent
            .spawn((
                UiTree::new(self.subtree.take().unwrap()),
                Name::new(format!("UiTree ({})", self.label)),
            ))
            .id();

        SubtreeWidget {
            entity: e,
            parent: parent.target_entity(),
        }
    }

    fn rebuild(&mut self, prev: &Self, widget: &mut Self::Widget, mut commands: Commands) {
        if self.label != prev.label {
            commands
                .entity(widget.entity())
                .despawn_related::<Children>()
                .insert((UiTree::new(self.subtree.take().unwrap()), 
                Name::new(format!("UiTree ({})", self.label)),));
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
