use anyhow::bail;
use bevy::prelude::*;

use crate::ui::style::{ConditionalStyle, Style, StyleLabel, StyleRef};

use super::{View, Widget};

pub struct Stylable<V: View> {
    pub style_ref: StyleRef,
    pub style_override: Style,
    pub conditional: ConditionalStyle,
    pub inner: V,
}

impl<V: View> Stylable<V> {
    pub fn new(inner: V) -> Self {
        Self {
            style_ref: default(),
            style_override: default(),
            conditional: ConditionalStyle::new(),
            inner,
        }
    }
}

impl<V: View> Stylable<V> {
    fn style_ref(&mut self, label: impl StyleLabel) -> &mut Self {
        self.style_ref = StyleRef::new(label);
        self
    }

    pub fn style<T: Reflect + TypePath>(
        &mut self,
        item: &str,
        value: T,
    ) -> anyhow::Result<&mut Self> {
        let value_path = value.reflect_type_path().to_string();
        match item {
            "background_color" => {
                if let Ok(color) = <dyn Reflect>::downcast(Box::new(value)) {
                    self.style_override.background_color = Some(*color);
                } else {
                    bail!("Background color takes Color value, not {value_path}");
                }
            }
            "border_color" => {
                if let Ok(color) = <dyn Reflect>::downcast(Box::new(value)) {
                    self.style_override.border_color = Some(*color);
                } else {
                    bail!("Border color takes Color value, not {value_path}");
                }
            }
            _ => {
                let node = Node::default();
                let Some(field) = node.field(item) else {
                    bail!("Style \"{item}\" doesn't exist");
                };
                if !field.represents::<T>() {
                    bail!(
                        "Style \"{item}\" takes a value of type {}, not {value_path}",
                        field.reflect_type_path()
                    );
                }
                self.style_override.node_stuff.insert(item, value);
            }
        }
        Ok(self)
    }

    pub fn with<T: Reflect + TypePath>(mut self, item: &str, value: T) -> anyhow::Result<Self> {
        self.style(item, value)?;
        Ok(self)
    }
}

macro impl_styles($($name:ident : $ty:ty),* $(,)?) {
    impl<V: View> Stylable<V> {
        $(
            pub fn $name(self, value: $ty) -> Self {
                self.with(stringify!($name), value).unwrap()
            }
        )*
    }
}

impl_styles! {
    background_color: Color,
    border_color: Color,

    display: Display,
    box_sizing: BoxSizing,
    position_type: PositionType,
    overflow: Overflow,
    overflow_clip_margin: OverflowClipMargin,
    left: Val,
    right: Val,
    top: Val,
    bottom: Val,
    width: Val,
    height: Val,
    min_width: Val,
    min_height: Val,
    max_width: Val,
    max_height: Val,
    aspect_ratio: Option<f32>,
    align_items: AlignItems,
    justify_items: JustifyItems,
    align_self: AlignSelf,
    justify_self: JustifySelf,
    align_content: AlignContent,
    justify_content: JustifyContent,
    margin: UiRect,
    padding: UiRect,
    border: UiRect,
    flex_direction: FlexDirection,
    flex_wrap: FlexWrap,
    flex_grow: f32,
    flex_shrink: f32,
    flex_basis: Val,
    row_gap: Val,
    column_gap: Val,
    grid_auto_flow: GridAutoFlow,
    grid_template_rows: Vec<RepeatedGridTrack>,
    grid_template_columns: Vec<RepeatedGridTrack>,
    grid_auto_rows: Vec<GridTrack>,
    grid_auto_columns: Vec<GridTrack>,
    grid_row: GridPlacement,
    grid_column: GridPlacement,
}

impl<V: View> View for Stylable<V> {
    type Widget = StylableWidget<V::Widget>;

    fn build(&mut self, parent: &mut ChildSpawnerCommands) -> Self::Widget {
        let widget = self.inner.build(parent);
        let mut commands = parent.commands();
        let mut e = commands.entity(widget.entity());

        e.insert(self.style_ref.clone());
        if !self.style_override.is_empty() {
            e.insert(self.style_override.clone());
        }
        if !self.conditional.x.is_empty() {
            e.insert(self.conditional.clone());
        }

        StylableWidget { inner: widget }
    }

    fn rebuild(&mut self, prev: &Self, widget: &mut Self::Widget, mut commands: Commands) {
        self.inner
            .rebuild(&prev.inner, &mut widget.inner, commands.reborrow());
        let mut entity = commands.entity(widget.entity());
        if self.style_ref != prev.style_ref {
            entity.insert(self.style_ref.clone());
        }
        if self.style_override != prev.style_override {
            entity.insert(self.style_override.clone());
        }
    }
}

pub struct StylableWidget<W: Widget> {
    inner: W,
}

impl<W: Widget> Widget for StylableWidget<W> {
    fn entity(&self) -> Entity {
        self.inner.entity()
    }

    fn parent(&self) -> Entity {
        self.inner.parent()
    }
}
