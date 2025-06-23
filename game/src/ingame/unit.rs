use super::{
    lua::{AppLuaExt, AssetPathExt, LuaExt, Protos},
    map::MapEntity,
    structure::Model,
    targetable::{Facing, Position},
};
use crate::{
    AppExt, GameState,
    ingame::{
        lua::{LuaCtx, W},
        targetable::Health,
        unit::{
            attack::{AutoAttackTarget, AutoAttackTimer, AutoAttackType},
            effect::{CustomData, EffectList},
            state::StateList,
            stats::{BaseStats, StatBlock},
        },
        vision::{SightRange, VisibleBy},
    },
};
use anyhow::anyhow;
use bevy::{
    asset::{AssetPath, uuid::Uuid},
    platform::collections::HashMap,
    prelude::*,
};
use lightyear::prelude::*;
use lobby_common::Team;
use mlua::prelude::*;

pub mod animation;
pub mod attack;
pub mod champion;
pub mod collision;
pub mod effect;
pub mod movement;
pub mod state;
pub mod stats;

pub fn common(app: &mut App) {
    app.add_plugins((
        movement::plugin,
        animation::plugin,
        effect::plugin,
        stats::plugin,
        champion::plugin,
        attack::plugin,
        state::plugin,
        collision::plugin,
    ));

    app.register_trigger::<SetUnitMovementTarget>(ChannelDirection::ClientToServer);
    app.register_component::<MovementTarget>(ChannelDirection::ServerToClient);
    app.register_component::<Unit>(ChannelDirection::ServerToClient);
    app.register_component::<UnitId>(ChannelDirection::ServerToClient);
    app.register_component::<UnitType>(ChannelDirection::ServerToClient);
    app.register_component::<ControlledByClient>(ChannelDirection::ServerToClient);

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

            let id = SpawnUnit(args).apply_from_lua(lua);

            UnitProxy { entity: id }.into_lua_multi(lua)
        })?,
    )?;

    Ok(())
}

pub struct SpawnUnit(pub SpawnUnitArgs);

impl SpawnUnit {
    fn spawn(self, world: &mut World) -> (UnitProto, Entity) {
        let args = self.0;

        let (proto, origin) = world
            .resource_mut::<Protos<UnitProto>>()
            .get(&args.proto)
            .unwrap();

        let mut attack_type = proto.attack_type.clone();
        if let AutoAttackType::Projectile(ref mut proto_cont) = attack_type
            && let Some(proj_proto) = proto.projectile_proto.clone()
        {
            *proto_cont = Some(proj_proto);
        }

        let id = world
            .spawn((
                Transform::from_translation(Position(args.position).into()),
                Position(args.position),
                Model(proto.model.clone().relative(&origin)),
                Unit,
                args.team,
                args.data,
                proto.unit_type,
                attack_type,
                MapEntity,
                Health(proto.base_stats.max_health),
                StatBlock::from(proto.base_stats.clone()),
                UnitId(Uuid::new_v4()),
                StateScoped(GameState::InGame),
                ServerReplicate::default(),
            ))
            .id();

        (proto, id)
    }

    fn apply_from_lua(self, lua: &Lua) -> Entity {
        let mut world = lua.world();

        let (proto, id) = self.spawn(&mut world);

        if let Some(on_spawn) = proto.on_spawn {
            drop(world);
            on_spawn.call::<()>(UnitProxy { entity: id }).unwrap();
        }

        id
    }
}

impl Command<Entity> for SpawnUnit {
    fn apply(self, world: &mut World) -> Entity {
        let (proto, id) = self.spawn(world);

        if let Some(on_spawn) = proto.on_spawn {
            let lua = world.resource::<LuaCtx>().0.clone();

            lua.with_world(world, |_lua| {
                on_spawn.call::<()>(UnitProxy { entity: id }).unwrap();
            });
        }

        id
    }
}

#[derive(Debug, Component, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UnitId(pub Uuid);

pub struct SpawnUnitArgs {
    pub proto: String,
    pub position: Vec2,
    pub team: Team,
    pub data: CustomData,
}

from_into_lua_table!(
    struct SpawnUnitArgs {
        proto: String,
        position: {W} Vec2,
        team: {W} Team,
        data: CustomData,
    }
);

impl FromLua for W<Team> {
    fn from_lua(value: LuaValue, _lua: &Lua) -> LuaResult<Self> {
        value
            .as_integer()
            .ok_or_else(|| anyhow!("Expected integer"))
            .and_then(|i| i.try_into().map_err(anyhow::Error::from))
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

#[derive(PartialEq)]
struct UnitProto {
    id: String,
    name: String,
    unit_type: UnitType,
    attack_type: AutoAttackType,
    projectile_proto: Option<String>,
    base_stats: BaseStats,
    model: AssetPath<'static>,
    on_spawn: Option<LuaFunction>,
}

proto!(
    struct UnitProto {
        id: String,
        name: String,
        unit_type: UnitType,
        attack_type: AutoAttackType,
        projectile_proto: Option<String>,
        base_stats: BaseStats,
        model: {W} AssetPath<'static>,
        on_spawn: Option<LuaFunction>,
    }
);

/// Marker struct for units
#[derive(Component, Default, Clone, PartialEq, Serialize, Deserialize)]
#[require(VisibleBy, SightRange = SightRange(10.0), EffectList, AutoAttackTimer, StateList, CustomData, Facing)]
pub struct Unit;

/// Where this unit currently wants to go
#[derive(Component, Clone, PartialEq, Serialize, Deserialize)]
pub struct MovementTarget(pub Position);

/// Message for clients to set their controlled unit's movement target.
#[derive(Debug, Event, Clone, Serialize, Deserialize)]
pub struct SetUnitMovementTarget(Position);

#[derive(Component, Clone, PartialEq, Serialize, Deserialize)]
pub struct ControlledByClient(pub ClientId);

#[derive(Component, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnitType {
    Normal,
    Champion,
}

impl FromLua for UnitType {
    fn from_lua(value: LuaValue, _lua: &Lua) -> LuaResult<Self> {
        match value.to_string()?.as_str() {
            "normal" => Ok(Self::Normal),
            "champion" => Ok(Self::Champion),
            other => Err(LuaError::external(anyhow!(
                "{other} is not a valid unit type"
            ))),
        }
    }
}

impl IntoLua for UnitType {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        lua.create_string(match self {
            UnitType::Normal => "normal",
            UnitType::Champion => "champion",
        })?
        .into_lua(lua)
    }
}

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

#[derive(Clone, FromLua)]
pub struct UnitProxy {
    pub entity: Entity,
}

impl LuaUserData for UnitProxy {
    fn add_fields<F: LuaUserDataFields<Self>>(_fields: &mut F) {}

    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("get_position", |lua, s, ()| {
            Ok(W(lua.world().entity(s.entity).get::<Position>().unwrap().0))
        });

        methods.add_method("set_position", |lua, s, pos: W<Vec2>| {
            lua.world()
                .entity_mut(s.entity)
                .get_mut::<Position>()
                .unwrap()
                .0 = pos.0;
            Ok(())
        });

        methods.add_method("set_movement_target", |lua, s, pos: W<Vec2>| {
            lua.world()
                .entity_mut(s.entity)
                .insert(MovementTarget(Position(pos.0)));
            Ok(())
        });

        methods.add_method("set_attack_target", |lua, s, target: UnitProxy| {
            lua.world()
                .entity_mut(s.entity)
                .insert(AutoAttackTarget(target.entity));
            Ok(())
        });

        methods.add_method("get_team", |lua, s, ()| {
            Ok(W(*lua.world().entity(s.entity).get::<Team>().unwrap()))
        });

        methods.add_method("get_visible_units", |lua, s, ()| {
            let mut world = lua.world();
            let unit_team = *world.entity(s.entity).get::<Team>().unwrap();
            let mut q = world.query_filtered::<(Entity, &Team, &VisibleBy), With<Unit>>();
            let units = q
                .iter(&world)
                .filter(|x| x.2.0.contains(&unit_team))
                .map(|x| UnitProxy { entity: x.0 });

            Ok(Vec::from_iter(units))
        });

        methods.add_method("apply_effect", effect::apply_effect);

        methods.add_method("get_custom_data", |lua, s, ()| {
            Ok(lua
                .world()
                .entity(s.entity)
                .get::<CustomData>()
                .unwrap()
                .clone())
        });

        methods.add_method("set_custom_data", |lua, s, data: CustomData| {
            *lua.world()
                .entity_mut(s.entity)
                .get_mut::<CustomData>()
                .unwrap() = data;
            Ok(())
        });
    }
}
