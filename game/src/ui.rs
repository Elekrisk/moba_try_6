use bevy::prelude::*;

pub mod style;

pub fn client(app: &mut App) {
    app.add_plugins((style::client,));
}
