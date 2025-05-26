use bevy::{ecs::bundle::BundleEffect, prelude::*};

use super::custom_effect;

pub fn client(app: &mut App) {}

pub fn text(txt: impl Into<String>) -> TextBuilder {
    TextBuilder::new(txt)
}

pub struct TextBuilder {
    text: String,
    font: Option<String>,
    size: f32,
}

impl TextBuilder {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            font: None,
            size: 20.0,
        }
    }

    pub fn font(mut self, path: impl Into<String>) -> Self {
        self.font = Some(path.into());
        self
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }
}

custom_effect!(TextBuilder);

impl BundleEffect for TextBuilder {
    fn apply(self, entity: &mut EntityWorldMut) {
        entity.insert(Text::new(self.text));
        let handle = match self.font {
            Some(path) => entity.resource::<AssetServer>().load(path),
            None => default(),
        };
        entity.insert(TextFont {
            font: handle,
            font_size: self.size,
            ..default()
        });
    }
}
