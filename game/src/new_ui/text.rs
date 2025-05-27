use std::fmt::Debug;

use super::{Pod, View, Widget};
use bevy::prelude::*;

#[derive(Debug)]
pub struct TextView {
    pub text: String,
}

impl TextView {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

pub trait TextViewable: Debug + Send + Sync + 'static {
    fn text(&self) -> &str;
}

impl TextViewable for TextView {
    fn text(&self) -> &str {
        &self.text
    }
}

impl TextViewable for String {
    fn text(&self) -> &str {
        &self
    }
}

impl TextViewable for &'static str {
    fn text(&self) -> &str {
        self
    }
}

impl<T: TextViewable> View for T {
    type Widget = TextWidget;

    fn build(&mut self, parent: &mut ChildSpawnerCommands) -> Self::Widget {
        let entity = parent
            .spawn((Text::new(self.text()), Name::new(self.name())))
            .id();
        TextWidget {
            entity,
            parent: parent.target_entity(),
        }
    }

    fn rebuild(&mut self, prev: &Self, widget: &mut Self::Widget, mut commands: Commands) {
        if self.text() != prev.text() {
            let txt = self.text().to_string();
            commands
                .entity(widget.entity())
                .insert(Name::new(self.name()))
                .entry::<Text>()
                .or_default()
                .and_modify(|mut text| text.0 = txt);
        }
    }

    fn name(&self) -> String {
        let mut short_slice_end = self.text().len().min(16);
        while !self.text().is_char_boundary(short_slice_end) {
            short_slice_end += 1;
        }

        let slice = self.text().get(0..short_slice_end).unwrap();
        format!("Text ({})", slice)
    }
}

pub struct TextWidget {
    entity: Entity,
    parent: Entity,
}

impl Widget for TextWidget {
    fn entity(&self) -> Entity {
        self.entity
    }

    fn parent(&self) -> Entity {
        self.parent
    }
}
