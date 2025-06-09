use std::fmt::Debug;

use bevy::prelude::*;

use super::{View, Widget};

#[allow(dead_code)]
pub struct CustomView<T: Debug, W> {
    pub data: T,
    pub build: Box<
        dyn FnMut(&mut T, &mut ChildSpawnerCommands) -> CustomWidget<W> + Send + Sync + 'static,
    >,
    pub rebuild:
        Box<dyn FnMut(&mut T, &Self, &mut CustomWidget<W>, Commands) + Send + Sync + 'static>,
}

impl<T: Debug, W> Debug for CustomView<T, W> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CustomView")
            .field("data", &self.data)
            .field_with("build", |fmt| fmt.write_str("Opaque"))
            .field_with("rebuild", |fmt| fmt.write_str("Opaque"))
            .finish()
    }
}

impl<T: Debug + Send + Sync + 'static, W: Send + Sync + 'static> View for CustomView<T, W> {
    type Widget = CustomWidget<W>;

    fn build(&mut self, parent: &mut ChildSpawnerCommands) -> Self::Widget {
        (self.build)(&mut self.data, parent)
    }

    fn rebuild(&mut self, prev: &Self, widget: &mut Self::Widget, commands: Commands) {
        (self.rebuild)(&mut self.data, prev, widget, commands)
    }
}

pub struct CustomWidget<T> {
    pub data: T,
    pub entity: Entity,
    pub parent: Entity,
}

impl<T: Send + Sync + 'static> Widget for CustomWidget<T> {
    fn entity(&self) -> Entity {
        self.entity
    }

    fn parent(&self) -> Entity {
        self.parent
    }
}
