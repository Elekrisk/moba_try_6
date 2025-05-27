use std::fmt::Debug;
use std::{
    any::{Any, TypeId},
    marker::PhantomData,
};

use anyhow::bail;
use bevy::{ecs::component::ComponentId, prelude::*};
use scrollable::ScrollableView;
use stylable::Stylable;

use crate::ui::style::{Style, StyleLabel, StyleRef};

pub mod button;
pub mod container;
pub mod custom;
pub mod image;
pub mod list;
pub mod optional;
pub mod scrollable;
pub mod stylable;
pub mod subtree;
pub mod tabbed;
pub mod text;
pub mod tree;

pub fn client(app: &mut App) {
    app.add_plugins((
        tree::client,
        tabbed::client,
        button::client,
        scrollable::client,
    ));
}

#[derive(Debug, Default)]
pub struct Pod {
    pub style_ref: StyleRef,
    pub style_override: Style,
}

pub trait View: Debug + Any + Send + Sync {
    type Widget: Widget;

    fn build(&mut self, parent: &mut ChildSpawnerCommands) -> Self::Widget;
    fn rebuild(&mut self, prev: &Self, widget: &mut Self::Widget, commands: Commands);
    fn name(&self) -> String {
        "???".into()
    }
}

pub trait ErasedView: Debug + Any + Send + Sync {
    fn build_erased(&mut self, parent: &mut ChildSpawnerCommands) -> BoxedWidget;
    fn rebuild_erased(
        &mut self,
        prev: &dyn ErasedView,
        widget: &mut dyn ErasedWidget,
        commands: Commands,
    );
    fn structure_erased(&self) -> String;
}

impl<V: View> ErasedView for V
where
    V::Widget: ErasedWidget + Send + Sync,
{
    fn build_erased(&mut self, parent: &mut ChildSpawnerCommands) -> BoxedWidget {
        BoxedWidget {
            inner: Box::new(self.build(parent)),
        }
    }

    fn rebuild_erased(
        &mut self,
        prev: &dyn ErasedView,
        widget: &mut dyn ErasedWidget,
        commands: Commands,
    ) {
        self.rebuild(
            (prev as &dyn Any).downcast_ref().unwrap(),
            (widget as &mut dyn Any).downcast_mut().unwrap(),
            commands,
        );
    }

    fn structure_erased(&self) -> String {
        format!("Erased({})", self.name())
    }
}

#[derive(Debug)]
pub struct BoxedView {
    pub inner: Box<dyn ErasedView>,
}

impl BoxedView {
    pub fn new(inner: impl ErasedView) -> Self {
        Self {
            inner: Box::new(inner),
        }
    }
}

impl View for BoxedView {
    type Widget = BoxedWidget;

    fn build(&mut self, parent: &mut ChildSpawnerCommands) -> Self::Widget {
        self.inner.build_erased(parent)
    }

    fn rebuild(&mut self, prev: &Self, widget: &mut Self::Widget, mut commands: Commands) {
        if (*self.inner).type_id() == (*prev.inner).type_id() {
            self.inner
                .rebuild_erased(&*prev.inner, &mut *widget.inner, commands);
        } else {
            // First, build new widget
            let mut new_widget = None;
            commands.entity(widget.parent()).with_children(|parent| {
                new_widget = Some(self.inner.build_erased(parent));
            });
            let new_widget = new_widget.unwrap();

            let new_widget_entity = new_widget.entity();
            let entity_to_despawn = widget.entity();
            let parent_entity = widget.parent();

            // Overwrite old widget with new
            *widget = new_widget;

            // We need to change the order of the children so that the newly created Node
            // is in the correct spot in the layout
            commands.queue(move |world: &mut World| {
                // We need to get the index of the child in the parent
                let children = world.get_mut::<Children>(parent_entity).unwrap();
                let index = children
                    .iter()
                    .position(|e| e == entity_to_despawn)
                    .unwrap();
                world.entity_mut(entity_to_despawn).despawn();
                world
                    .entity_mut(parent_entity)
                    .insert_children(index, &[new_widget_entity]);
            });
        }
    }

    fn name(&self) -> String {
        format!("Boxed({})", self.inner.structure_erased())
    }
}

impl View for () {
    type Widget = (Entity, Entity);

    fn build(&mut self, parent: &mut ChildSpawnerCommands) -> Self::Widget {
        (parent.spawn_empty().id(), parent.target_entity())
    }

    fn rebuild(&mut self, prev: &Self, widget: &mut Self::Widget, commands: Commands) {}
}

pub trait Widget: Any + Send + Sync {
    fn despawn(&self, mut commands: Commands) {
        let entity = self.entity();
        if entity != Entity::PLACEHOLDER {
            commands.entity(entity).despawn();
        }
    }
    fn entity(&self) -> Entity;
    fn parent(&self) -> Entity;
}

pub trait ErasedWidget: Any {
    fn despawn_erased(&self, commands: Commands);
    fn entity_erased(&self) -> Entity;
    fn parent_erased(&self) -> Entity;
    // fn set(&mut self, other: Box<dyn Widget>);
}

impl<W: Widget> ErasedWidget for W {
    fn despawn_erased(&self, commands: Commands) {
        self.despawn(commands);
    }

    fn entity_erased(&self) -> Entity {
        self.entity()
    }

    fn parent_erased(&self) -> Entity {
        self.parent()
    }

    // fn set(&mut self, other: Box<dyn Widget>) {
    //     *self = *(other as Box<dyn Any>).downcast().unwrap();
    // }
}

pub struct BoxedWidget {
    pub inner: Box<dyn ErasedWidget + Send + Sync>,
}

impl Widget for BoxedWidget {
    fn despawn(&self, commands: Commands) {
        self.inner.despawn_erased(commands);
    }

    fn entity(&self) -> Entity {
        self.inner.entity_erased()
    }

    fn parent(&self) -> Entity {
        self.inner.parent_erased()
    }
}

impl Widget for (Entity, Entity) {
    fn entity(&self) -> Entity {
        self.0
    }

    fn parent(&self) -> Entity {
        self.1
    }
}

pub trait ViewExt: View + Sized {
    fn styled(self) -> Stylable<Self> {
        Stylable::new(self)
    }

    fn scrollable(self) -> ScrollableView<Self> {
        ScrollableView { inner: self }
    }

    fn boxed(self) -> BoxedView {
        BoxedView::new(self)
    }
}

impl<V: View> ViewExt for V {}

pub trait WidgetExt: Any + Sized {}

impl<W: ErasedWidget> WidgetExt for W {}
