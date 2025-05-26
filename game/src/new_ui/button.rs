use bevy::{
    ecs::system::{IntoObserverSystem, ObserverSystem, SystemId},
    prelude::*,
};

use crate::ui::style::{
    ComponentEquals, ConditionalStyle, DefaultStyle, StyleLabel, StyleRef, Styles, style_label,
};

use super::{View, Widget};

pub fn client(app: &mut App) {
    app.add_systems(Startup, setup_styles);
}

style_label!(DefaultButtonStyle);
style_label!(HoverButtonStyle);
style_label!(PressButtonStyle);

pub fn setup_styles(mut styles: ResMut<Styles>) {
    let style = styles
        .new_style_from(DefaultStyle, DefaultButtonStyle)
        .unwrap();
    style.background_color = Some(Color::srgb(0.3, 0.3, 0.3));
    style.border_color = Some(Color::WHITE);
    style.set_node_field("border", UiRect::all(Val::Px(1.0)));
    style.set_node_field("padding", UiRect::all(Val::Px(2.0)));

    let style = styles
        .new_style_from(DefaultButtonStyle, HoverButtonStyle)
        .unwrap();
    style.background_color = Some(Color::srgb(0.4, 0.4, 0.4));

    let style = styles
        .new_style_from(DefaultButtonStyle, PressButtonStyle)
        .unwrap();
    style.background_color = Some(Color::srgb(0.2, 0.2, 0.2));
}

pub struct ButtonView<V: View, S: ObserverSystem<Pointer<Click>, ()>> {
    inner: V,
    callback_label: String,
    on_click: Option<S>,
    pub normal_style: StyleRef,
    pub hover_style: StyleRef,
    pub press_style: StyleRef,
}

impl<V: View, S: ObserverSystem<Pointer<Click>, ()>> ButtonView<V, S> {
    pub fn new<M>(
        inner: V,
        label: impl Into<String>,
        on_click: impl ButtonCallback<M, System = S>,
    ) -> Self {
        Self {
            inner,
            callback_label: label.into(),
            on_click: Some(on_click.to_observer_system()),
            normal_style: StyleRef::new(DefaultButtonStyle),
            hover_style: StyleRef::new(HoverButtonStyle),
            press_style: StyleRef::new(PressButtonStyle),
        }
    }
}

pub trait ButtonCallback<M> {
    type System: ObserverSystem<Pointer<Click>, ()>;

    fn to_observer_system(self) -> Self::System;
}

impl<M, S: IntoObserverSystem<Pointer<Click>, (), M>> ButtonCallback<(i32, M)> for S {
    type System = <Self as IntoObserverSystem<Pointer<Click>, (), M>>::System;

    fn to_observer_system(self) -> Self::System {
        IntoObserverSystem::into_system(self)
    }
}

impl<M, S: IntoSystem<(), (), M> + Send + Sync + 'static> ButtonCallback<(i64, M)> for S {
    type System = impl ObserverSystem<Pointer<Click>, ()>;

    fn to_observer_system(self) -> Self::System {
        let mut on_click = Some(self);
        IntoObserverSystem::<_, (), _>::into_system(
            move |mut trigger: Trigger<Pointer<Click>>,
                  mut id: Local<Option<SystemId>>,
                  mut commands: Commands| {
                trigger.propagate(false);
                if id.is_none() {
                    *id = Some(commands.register_system(on_click.take().unwrap()));
                }
                commands.run_system(id.unwrap());
            },
        )
    }
}

impl<V: View, S: ObserverSystem<Pointer<Click>, ()>> View for ButtonView<V, S> {
    type Widget = ButtonWidget<V::Widget>;

    fn build(&mut self, parent: &mut ChildSpawnerCommands) -> Self::Widget {
        let mut inner = None;
        let on_click = self.on_click.take().unwrap();
        let e = parent
            .spawn((
                Button,
                ConditionalStyle::new()
                    .when(ComponentEquals(Interaction::None), DefaultButtonStyle)
                    .when(ComponentEquals(Interaction::Hovered), HoverButtonStyle)
                    .when(ComponentEquals(Interaction::Pressed), PressButtonStyle),
                Name::new(format!("Button ({})", &self.callback_label)),
            ))
            .with_children(|parent| {
                inner = Some(self.inner.build(parent));
            })
            .id();
        let observer = parent
            .commands()
            .spawn(Observer::new(on_click).with_entity(e))
            .id();
        ButtonWidget {
            entity: e,
            parent: parent.target_entity(),
            observer,
            inner: inner.unwrap(),
        }
    }

    fn rebuild(&mut self, prev: &Self, widget: &mut Self::Widget, mut commands: Commands) {
        self.inner
            .rebuild(&prev.inner, &mut widget.inner, commands.reborrow());
        if self.callback_label != prev.callback_label {
            commands
                .entity(widget.entity)
                .insert(Name::new(format!("Button ({})", &self.callback_label)));
            // Rebuild callback
            commands.entity(widget.observer).despawn();
            let on_click = self.on_click.take().unwrap();
            widget.observer = commands
                .spawn(Observer::new(on_click).with_entity(widget.entity))
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
