use bevy::prelude::*;
use lightyear::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Component, Serialize, Deserialize)]
pub struct Health(pub f32);

pub fn common(app: &mut App) {
    app.add_systems(Update, kill_if_low_health)
        .register_component::<Health>(ChannelDirection::ServerToClient);
}

fn kill_if_low_health(query: Query<(Entity, &Health, /*Option<&LuaObject>*/)>) {
    for (_e, hp, /*obj*/) in &query {
        if hp.0 <= 0.0 {}
        // Kill!
        // if let Some(_obj) = obj {
        //     // obj.0.get()
        // }
    }
}