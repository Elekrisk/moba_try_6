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

impl View for TextView {
    type Widget = TextWidget;

    fn build(&self, parent: &mut ChildSpawnerCommands) -> Self::Widget {
        let entity = parent.spawn(Text::new(&self.text)).id();
        TextWidget {
            entity,
            parent: parent.target_entity(),
        }
    }

    fn rebuild(&self, prev: &Self, widget: &mut Self::Widget, mut commands: Commands) {
        if self.text != prev.text {
            let txt = self.text.clone();
            commands
                .entity(widget.entity())
                .entry::<Text>()
                .or_default()
                .and_modify(|mut text| text.0 = txt);
        }
    }

    fn structure(&self) -> String {
        "TextView".into()
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
