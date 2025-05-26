use bevy::{ecs::{system::SystemId, world::CommandQueue}, prelude::*};

use super::{BoxedView, BoxedWidget, ErasedView, View};

pub fn client(app: &mut App) {
    app.add_systems(Update, weee);
}

pub trait UiFunc: Send + Sync + 'static {
    fn run(&mut self, entity: Entity, world: &mut World) -> Option<BoxedView>;
}

pub struct UiFuncSystem<V: ErasedView, S: System<In = (), Out = Option<V>>>(S, Option<SystemId<(), Option<V>>>);

impl<V: ErasedView, S: System<In = (), Out = Option<V>> + Clone> Clone for UiFuncSystem<V, S> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), self.1.clone())
    }
}

impl<V: ErasedView, S: System<In = (), Out = Option<V>> + Clone> UiFunc for UiFuncSystem<V, S> {
    fn run(&mut self, entity: Entity, world: &mut World) -> Option<BoxedView> {
        if self.1.is_none() {
            self.1 = Some(world.register_system(self.0.clone()));
        }
        world.run_system(self.1.unwrap()).unwrap().map(BoxedView::new)
    }
}

pub struct UiFuncSystemIn<V: ErasedView, S: System<In = In<Entity>, Out = Option<V>>>(S, Option<SystemId<In<Entity>, Option<V>>>);

impl<V: ErasedView, S: System<In = In<Entity>, Out = Option<V>> + Clone> Clone for UiFuncSystemIn<V, S> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), self.1.clone())
    }
}

impl<V: ErasedView, S: System<In = In<Entity>, Out = Option<V>> + Clone> UiFunc for UiFuncSystemIn<V, S> {
    fn run(&mut self, entity: Entity, world: &mut World) -> Option<BoxedView> {
        if self.1.is_none() {
            self.1 = Some(world.register_system(self.0.clone()));
        }
        world.run_system_with(self.1.unwrap(), entity).unwrap().map(BoxedView::new)
    }
}

pub trait IntoUiFunc<M> {
    type UiFunc: UiFunc;

    fn into_ui_func(self) -> Self::UiFunc;
}

impl<F: UiFunc> IntoUiFunc<()> for F {
    type UiFunc = Self;

    fn into_ui_func(self) -> Self::UiFunc {
        self
    }
}

impl<V: ErasedView, M, S: IntoSystem<(), Option<V>, M, System: Clone> + Send + Sync + 'static>
    IntoUiFunc<(u32, M, V)> for S
{
    type UiFunc = UiFuncSystem<V, S::System>;

    fn into_ui_func(self) -> Self::UiFunc {
        UiFuncSystem(IntoSystem::into_system(self), None)
    }
}

#[derive(Component)]
#[require(Node)]
pub struct UiTree {
    ui: Option<Box<dyn UiFunc>>,
    widget: Option<BoxedWidget>,
    prev: Option<BoxedView>,
}

impl UiTree {
    pub fn new<M>(f: impl IntoUiFunc<M>) -> Self {
        Self {
            ui: Some(Box::new(f.into_ui_func())),
            widget: None,
            prev: None,
        }
    }
}

fn weee(world: &mut World) {
    let mut q = world.query_filtered::<Entity, With<UiTree>>();
    let mut command_queue = CommandQueue::default();
    let mut commands = Commands::new(&mut command_queue, world);
    for e in q.iter(world) {
        commands.queue(move |world: &mut World| {
            let mut entity = world.entity_mut(e);
            let mut tree = entity.get_mut::<UiTree>().unwrap();
            let mut ui = tree.ui.take().unwrap();
            let new_tree = ui.run(e, world);
            let Some(new_tree) = new_tree else {
                let mut tree = world.get_mut::<UiTree>(e).unwrap();
                tree.ui = Some(ui);
                return;
            };

            let (mut entities, mut commands) = world.entities_and_commands();
            let mut entity = entities.get_mut(e).unwrap();
            let tree = entity.get_mut::<UiTree>().unwrap();
            let tree = tree.into_inner();
            if let Some(prev) = tree.prev.as_ref() {
                let widget = tree.widget.as_mut().unwrap();
                new_tree.rebuild(prev, widget, commands);
                tree.prev = Some(new_tree);
            } else {
                commands.entity(e).with_children(|parent| {
                    let widget = new_tree.build(parent);
                    let entity = parent.target_entity();
                    parent
                        .commands()
                        .entity(entity)
                        .entry::<UiTree>()
                        .and_modify(|mut tree| {
                            tree.prev = Some(new_tree);
                            tree.widget = Some(widget);
                        });
                });
            }

            tree.ui = Some(ui);
        });
    }
    command_queue.apply(world);
}