use bevy::prelude::*;

use super::{View, Widget};

pub struct CustomView<
    B: FnMut(&mut ChildSpawnerCommands) -> CustomWidget,
    R: FnMut(&Self, &mut CustomWidget, Commands),
> {
    build: B,
    rebuild: R,
}

impl<
    B: FnMut(&mut ChildSpawnerCommands) -> CustomWidget + Send + Sync + 'static,
    R: FnMut(&Self, &mut CustomWidget, Commands) + Send + Sync + 'static,
> View for CustomView<B, R>
{
    type Widget = CustomWidget;

    fn build(&mut self, parent: &mut ChildSpawnerCommands) -> Self::Widget {
        (self.build)(parent)
    }

    fn rebuild(&mut self, prev: &Self, widget: &mut Self::Widget, commands: Commands) {
        (self.rebuild)(prev, widget, commands)
    }
}

pub struct CustomWidget {
    entity: Entity,
    parent: Entity,
}

impl Widget for CustomWidget {
    fn entity(&self) -> Entity {
        self.entity
    }

    fn parent(&self) -> Entity {
        self.parent
    }
}
