use std::path::PathBuf;

use bevy::prelude::*;

use super::{View, Widget};

#[derive(Debug)]
pub struct ImageView {
    img: PathBuf,
}

impl ImageView {
    pub fn new(img: impl Into<PathBuf>) -> Self {
        Self { img: img.into() }
    }
}

impl View for ImageView {
    type Widget = ImageWidget;

    fn build(&mut self, parent: &mut ChildSpawnerCommands) -> Self::Widget {
        let path = self.img.clone();

        let entity = parent
            .spawn((ImageNode::default(),))
            .queue(move |mut entity: EntityWorldMut<'_>| {
                let img = entity.resource::<AssetServer>().load(path);
                entity.insert(ImageNode::new(img));
            })
            .id();

        ImageWidget {
            entity,
            parent: parent.target_entity(),
        }
    }

    fn rebuild(&mut self, prev: &Self, widget: &mut Self::Widget, mut commands: Commands) {
        if self.img != prev.img {
            let path = self.img.clone();
            commands
                .entity(widget.entity)
                .queue(move |mut entity: EntityWorldMut<'_>| {
                    let img = entity.resource::<AssetServer>().load(path);
                    entity.insert(ImageNode::new(img));
                });
        }
    }

    fn name(&self) -> String {
        format!("Image ({})", self.img.display())
    }
}

pub struct ImageWidget {
    entity: Entity,
    parent: Entity,
}

impl Widget for ImageWidget {
    fn entity(&self) -> Entity {
        self.entity
    }

    fn parent(&self) -> Entity {
        self.parent
    }
}
