use super::{
    lua::{AppLuaExt, AssetPathExt, LuaExt, Protos},
    map::MapEntity,
    structure::Model,
};
use crate::{
    ingame::{lua::W, vision::{SightRange, VisibleBy}}, AppExt
};
use anyhow::anyhow;
use bevy::{asset::AssetPath, prelude::*};
use bevy_enhanced_input::prelude::*;
use lightyear::prelude::*;
use lobby_common::Team;
use mlua::prelude::*;

mod animation;
mod movement;

pub fn common(app: &mut App) {
    app.add_plugins((movement::plugin, animation::plugin));

    app.register_trigger::<SetUnitMovementTarget>(ChannelDirection::ClientToServer);
    app.register_component::<MovementTarget>(ChannelDirection::ServerToClient);
    app.register_component::<Unit>(ChannelDirection::ServerToClient);

    app.init_resource::<Protos<UnitProto>>();

    app.setup_lua(setup_lua);
}

fn setup_lua(lua: &Lua) -> LuaResult<()> {
    let game = lua.table("game")?;

    game.set(
        "register_unit",
        lua.create_function(|lua, proto: LuaValue| {
            lua.world()
                .resource_mut::<Protos<UnitProto>>()
                .insert(proto, lua.current_path())
        })?,
    )?;

    game.set(
        "spawn_unit",
        lua.create_function(|lua, args: SpawnUnitArgs| {
            if lua.is_client() {
                return Ok(());
            }

            let mut world = lua.world();

            let (proto, origin) = world.resource_mut::<Protos<UnitProto>>().get(&args.proto)?;

            world.spawn((
                Transform::from_xyz(args.position.x, 0.0, args.position.y),
                Model(proto.model.relative(&origin)),
                Unit,
                // MovementTarget(vec2(0.0, 0.0)),
                // VisualInterpolateStatus::<Transform>::default(),
                args.team,
                MapEntity,
                ServerReplicate::default(),
            ));

            Ok(())
        })?,
    )?;

    Ok(())
}

struct SpawnUnitArgs {
    proto: String,
    position: Vec2,
    team: Team,
}

from_into_lua_table!(
    struct SpawnUnitArgs {
        proto: String,
        position: {W} Vec2,
        team: {W} Team,
    }
);

impl FromLua for W<Team> {
    fn from_lua(value: LuaValue, _lua: &Lua) -> LuaResult<Self> {
        value
            .as_integer()
            .ok_or_else(|| anyhow!("Expected integer"))
            .map(|i| i.try_into().map_err(anyhow::Error::from))
            .flatten()
            .map(Team)
            .map(W)
            .map_err(LuaError::external)
    }
}

impl IntoLua for W<Team> {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        self.0.0.into_lua(lua)
    }
}

struct UnitProto {
    id: String,
    name: String,
    model: AssetPath<'static>,
}

proto!(
struct UnitProto {
    id: String,
    name: String,
    model: {W} AssetPath<'static>
});

/// Marker struct for units
#[derive(Component, Clone, PartialEq, Serialize, Deserialize)]
#[require(VisibleBy, SightRange = SightRange(10.0))]
pub struct Unit;

/// Where this unit currently wants to go
#[derive(Component, Clone, PartialEq, Serialize, Deserialize)]
pub struct MovementTarget(pub Vec2);

/// Message for clients to set their controlled unit's movement target.
#[derive(Debug, Event, Clone, Serialize, Deserialize)]
pub struct SetUnitMovementTarget(Vec2);

#[derive(Component, Clone, PartialEq, Serialize, Deserialize)]
pub struct ControlledByClient(pub ClientId);

#[derive(Resource, Clone, PartialEq, Serialize, Deserialize)]
pub struct MyTeam(pub Team);
