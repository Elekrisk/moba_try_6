use bevy::{prelude::*, ui::experimental::GhostNode};

use super::{View, Widget};

impl<V: View> View for Option<V> {
    type Widget = OptionalWidget<V::Widget>;

    fn build(&mut self, parent: &mut ChildSpawnerCommands) -> Self::Widget {
        let mut ghost = parent.spawn((GhostNode, Name::new(self.name()), Children::default()));
        let mut inner_widget = None;
        if let Some(inner) = self {
            ghost.with_children(|parent| {
                inner_widget = Some(inner.build(parent));
            });
        }

        OptionalWidget {
            entity: ghost.id(),
            parent: parent.target_entity(),
            inner: inner_widget,
        }
    }

    fn rebuild(&mut self, prev: &Self, widget: &mut Self::Widget, mut commands: Commands) {
        let name = self.name();
        match (self, prev) {
            (None, None) => {}
            (None, Some(_)) => {
                widget.inner.as_mut().unwrap().despawn(commands);
                widget.inner = None;
            }
            (Some(inner), None) => {
                let mut entity = commands.entity(widget.entity);
                entity.insert(Name::new(name));
                entity.with_children(|parent| {
                    widget.inner = Some(inner.build(parent));
                });
            }
            (Some(inner), Some(prev_inner)) => {
                inner.rebuild(
                    prev_inner,
                    widget.inner.as_mut().unwrap(),
                    commands.reborrow(),
                );
                commands.entity(widget.entity).insert(Name::new(name));
            }
        }
    }

    fn name(&self) -> String {
        match self {
            Some(inner) => format!("Optional ({})", inner.name()),
            None => "Optional (None)".into(),
        }
    }
}

pub struct OptionalWidget<W: Widget> {
    entity: Entity,
    parent: Entity,
    inner: Option<W>,
}

impl<W: Widget> Widget for OptionalWidget<W> {
    fn entity(&self) -> Entity {
        self.entity
    }

    fn parent(&self) -> Entity {
        self.parent
    }
}
