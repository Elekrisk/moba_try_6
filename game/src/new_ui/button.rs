use bevy::{ecs::system::SystemId, prelude::*};

use super::{View, Widget};

pub struct ButtonView<V: View, S: System<In = (), Out = ()> + Clone> {
    inner: V,
    callback_label: String,
    on_click: S,
}

pub enum ButtonAction {
    RunSystem(Box<dyn System<In = (), Out = ()>>),
}

impl<V: View, S: System<In = (), Out = ()> + Clone> ButtonView<V, S> {
    pub fn new<M>(
        inner: V,
        label: impl Into<String>,
        on_click: impl IntoSystem<(), (), M, System = S>,
    ) -> Self {
        Self {
            inner,
            callback_label: label.into(),
            on_click: IntoSystem::into_system(on_click),
        }
    }
}

const fn size_test<S>() {
    if core::mem::size_of::<S>() != 0 {
        panic!("Size test failed!");
    }
}

impl<V: View, S: System<In = (), Out = ()> + Clone> View for ButtonView<V, S> {
    type Widget = ButtonWidget<V::Widget>;

    fn build(&self, parent: &mut ChildSpawnerCommands) -> Self::Widget {
        let mut inner = None;
        let on_click = self.on_click.clone();
        let e = parent
            .spawn(Button)
            .with_children(|parent| {
                inner = Some(self.inner.build(parent));
            })
            .id();
        let observer = parent
            .commands()
            .spawn(
                Observer::new(
                    move |mut trigger: Trigger<Pointer<Click>>,
                          mut id: Local<Option<SystemId>>,
                          mut commands: Commands| {
                        trigger.propagate(false);
                        if id.is_none() {
                            *id = Some(commands.register_system(on_click.clone()));
                        }
                        commands.run_system(id.unwrap());
                    },
                )
                .with_entity(e),
            )
            .id();
        ButtonWidget {
            entity: e,
            parent: parent.target_entity(),
            observer,
            inner: inner.unwrap(),
        }
    }

    fn rebuild(&self, prev: &Self, widget: &mut Self::Widget, mut commands: Commands) {
        self.inner
            .rebuild(&prev.inner, &mut widget.inner, commands.reborrow());
        if self.callback_label != prev.callback_label {
            // Rebuild callback
            commands.entity(widget.observer).despawn();
            let on_click = self.on_click.clone();
            widget.observer = commands
                .spawn(
                    Observer::new(
                        move |mut trigger: Trigger<Pointer<Click>>,
                              mut id: Local<Option<SystemId>>,
                              mut commands: Commands| {
                            trigger.propagate(false);
                            if id.is_none() {
                                *id = Some(commands.register_system(on_click.clone()));
                            }
                            commands.run_system(id.unwrap());
                        },
                    )
                    .with_entity(widget.entity),
                )
                .id();
        }
    }
}

pub struct ButtonWidget<W: Widget> {
    entity: Entity,
    parent: Entity,
    observer: Entity,
    inner: W,
}

impl<W: Widget> Widget for ButtonWidget<W> {
    fn entity(&self) -> Entity {
        self.entity
    }

    fn parent(&self) -> Entity {
        self.parent
    }
}
