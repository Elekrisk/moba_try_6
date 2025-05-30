use bevy::{input::mouse::MouseScrollUnit, prelude::*};

use super::{View, Widget};

pub fn client(app: &mut App) {
    app.add_observer(
        |mut trigger: Trigger<Pointer<Scroll>>,
         mut q: Query<&mut ScrollPosition, With<Scrollable>>| {
            if let Ok(mut pos) = q.get_mut(trigger.target()) {
                pos.offset_y -= trigger.event().y
                    * match trigger.event().unit {
                        MouseScrollUnit::Line => 24.0,
                        MouseScrollUnit::Pixel => 1.0,
                    };
                trigger.propagate(false);
            }
        },
    );
}

#[derive(Debug)]
pub struct ScrollableView<V: View> {
    pub inner: V,
}

#[derive(Component)]
pub struct Scrollable;

impl<V: View> View for ScrollableView<V> {
    type Widget = ScrollableWidget<V::Widget>;

    fn build(&mut self, parent: &mut ChildSpawnerCommands) -> Self::Widget {
        let widget = self.inner.build(parent);
        parent
            .commands()
            .entity(widget.entity())
            .insert(Scrollable)
            .entry::<Node>()
            .or_default()
            .and_modify(|mut node| node.overflow = Overflow::scroll_y());

        ScrollableWidget { inner: widget }
    }

    fn rebuild(&mut self, prev: &Self, widget: &mut Self::Widget, commands: Commands) {
        self.inner.rebuild(&prev.inner, &mut widget.inner, commands);
    }

    fn name(&self) -> String {
        format!("{} C", self.inner.name())
    }
}

pub struct ScrollableWidget<W: Widget> {
    inner: W,
}

impl<W: Widget> Widget for ScrollableWidget<W> {
    fn entity(&self) -> Entity {
        self.inner.entity()
    }

    fn parent(&self) -> Entity {
        self.inner.parent()
    }
}
