use super::{
    lua::{AppLuaExt, AssetPathExt, LuaExt, Protos},
    map::MapEntity,
    structure::Model,
};
use crate::{
    ingame::{
        lua::W,
        targetable::Health,
        unit::{attack::AutoAttackTimer, effect::EffectList, stats::{BaseStats, StatBlock}},
        vision::{SightRange, VisibleBy},
    }, AppExt, GameState
};
use anyhow::anyhow;
use bevy::{
    asset::{AssetPath, uuid::Uuid},
    platform::collections::HashMap,
    prelude::*,
};
use bevy_enhanced_input::prelude::*;
use lightyear::prelude::*;
use lobby_common::Team;
use mlua::prelude::*;

pub mod animation;
pub mod champion;
pub mod effect;
pub mod movement;
pub mod stats;
pub mod attack;
pub mod state;

pub fn common(app: &mut App) {
    app.add_plugins((
        movement::plugin,
        animation::plugin,
        effect::plugin,
        stats::plugin,
        champion::plugin,
        attack::plugin,
        state::plugin,
    ));

    app.register_trigger::<SetUnitMovementTarget>(ChannelDirection::ClientToServer);
    app.register_component::<MovementTarget>(ChannelDirection::ServerToClient);
    app.register_component::<Unit>(ChannelDirection::ServerToClient);
    app.register_component::<UnitId>(ChannelDirection::ServerToClient);

    app.init_resource::<Protos<UnitProto>>();
    app.init_resource::<UnitMap>();

    app.add_observer(on_unit_id_inserted);
    app.add_observer(on_unit_id_replaced);
    app.add_observer(on_unit_id_removed);

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
                return ().into_lua_multi(lua);
            }

            let mut world = lua.world();

            let id = SpawnUnit(args).apply(&mut world);

            UnitProxy { entity: id }.into_lua_multi(lua)
        })?,
    )?;

    Ok(())
}

pub struct SpawnUnit(pub SpawnUnitArgs);

impl Command<Entity> for SpawnUnit {
    fn apply(self, world: &mut World) -> Entity {
        let args = self.0;
        
        let (proto, origin) = world.resource_mut::<Protos<UnitProto>>().get(&args.proto).unwrap();
        
        let id = world
            .spawn((
                Transform::from_xyz(args.position.x, 0.0, args.position.y),
                Model(proto.model.relative(&origin)),
                Unit,
                args.team,
                MapEntity,
                Health(proto.base_stats.max_health),
                StatBlock::from(proto.base_stats),
                UnitId(Uuid::new_v4()),
                StateScoped(GameState::InGame),
                ServerReplicate::default(),
            ))
            .id();

        id
    }
}

#[derive(Debug, Component, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UnitId(pub Uuid);

pub struct SpawnUnitArgs {
    pub proto: String,
    pub position: Vec2,
    pub team: Team,
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
    base_stats: BaseStats,
    model: AssetPath<'static>,
}

proto!(
struct UnitProto {
    id: String,
    name: String,
    base_stats: BaseStats,
    model: {W} AssetPath<'static>
});

/// Marker struct for units
#[derive(Component, Clone, PartialEq, Serialize, Deserialize)]
#[require(VisibleBy, SightRange = SightRange(10.0), EffectList, AutoAttackTimer)]
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

#[derive(Resource, Default)]
pub struct UnitMap {
    map: HashMap<UnitId, Entity>,
}

fn on_unit_id_inserted(
    trigger: Trigger<OnInsert, UnitId>,
    q: Query<&UnitId>,
    mut map: ResMut<UnitMap>,
) {
    map.map
        .insert(*q.get(trigger.target()).unwrap(), trigger.target());
}

fn on_unit_id_replaced(
    trigger: Trigger<OnReplace, UnitId>,
    q: Query<&UnitId>,
    mut map: ResMut<UnitMap>,
) {
    map.map.remove(q.get(trigger.target()).unwrap());
}

fn on_unit_id_removed(
    trigger: Trigger<OnRemove, UnitId>,
    q: Query<&UnitId>,
    mut map: ResMut<UnitMap>,
) {
    map.map.remove(q.get(trigger.target()).unwrap());
}

// --- Proxy ---

pub struct UnitProxy {
    pub entity: Entity,
}

impl LuaUserData for UnitProxy {
    fn add_fields<F: LuaUserDataFields<Self>>(_fields: &mut F) {}

    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("get_position", |lua, s, ()| {
            Ok(W(lua
                .world()
                .entity(s.entity)
                .get::<Transform>()
                .unwrap()
                .translation
                .xz()))
        });

        methods.add_method("set_position", |lua, s, pos: W<Vec2>| {
            lua.world()
                .entity_mut(s.entity)
                .get_mut::<Transform>()
                .unwrap()
                .translation = pos.extend(0.0).xzy();
            Ok(())
        });

        methods.add_method("set_movement_target", |lua, s, pos: W<Vec2>| {
            lua.world()
                .entity_mut(s.entity)
                .insert(MovementTarget(pos.0));
            Ok(())
        });

        methods.add_method("get_team", |lua, s, ()| {
            Ok(W(*lua.world().entity(s.entity).get::<Team>().unwrap()))
        });

        methods.add_method("apply_effect", effect::apply_effect);
    }
}
