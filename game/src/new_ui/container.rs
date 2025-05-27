pub use bevy::prelude::*;

use super::{View, Widget};

#[derive(Debug)]
pub struct ContainerView<V: View> {
    inner: V,
}

impl<V: View> ContainerView<V> {
    pub fn new(inner: V) -> Self {
        Self { inner }
    }
}

impl<V: View> View for ContainerView<V> {
    type Widget = ContainerWidget<V::Widget>;

    fn build(&mut self, parent: &mut ChildSpawnerCommands) -> Self::Widget {
        let mut widget = None;
        let entity = parent
            .spawn((Node::default(), Name::new(self.name())))
            .with_children(|parent| {
                widget = Some(self.inner.build(parent));
            })
            .id();

        ContainerWidget {
            entity,
            parent: parent.target_entity(),
            inner: widget.unwrap(),
        }
    }

    fn rebuild(&mut self, prev: &Self, widget: &mut Self::Widget, commands: Commands) {
        self.inner.rebuild(&prev.inner, &mut widget.inner, commands);
    }

    fn name(&self) -> String {
        format!("Container({})", self.inner.name())
    }
}

pub struct ContainerWidget<W: Widget> {
    entity: Entity,
    parent: Entity,
    inner: W,
}

impl<W: Widget> Widget for ContainerWidget<W> {
    fn entity(&self) -> Entity {
        self.entity
    }

    fn parent(&self) -> Entity {
        self.parent
    }
}
