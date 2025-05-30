use bevy::prelude::*;
use mlua::prelude::*;

#[derive(Resource, Deref)]
pub struct LuaCtx(pub Lua);

pub fn common(app: &mut App) {
    app.insert_resource(LuaCtx(Lua::new()));
}
