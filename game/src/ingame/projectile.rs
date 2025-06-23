use bevy::{asset::AssetPath, ecs::entity::MapEntities, prelude::*};
use lightyear::prelude::*;
use lobby_common::Team;
use mlua::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    ingame::{
        lua::{AppLuaExt, AssetPathExt, LuaCtx, LuaExt, Protos, W},
        map::MapEntity,
        structure::Model,
        targetable::{Facing, Position},
        unit::{effect::CustomData, UnitProxy}, vision::VisibleBy,
    }, AppExt, GameState
};

pub fn plugin(app: &mut App) {
    app.register_component::<Projectile>(ChannelDirection::ServerToClient);

    app.init_resource::<Protos<ProjectileProto>>();

    if app.is_server() {
        app.add_systems(FixedUpdate, move_projectile);
    }

    app.setup_lua(setup_lua);
}

#[derive(Component, PartialEq, Serialize, Deserialize, MapEntities)]
#[require(VisibleBy, CustomData, Facing)]
pub struct Projectile {
    pub target: ProjectileTarget,
    pub speed: f32,
}

#[derive(PartialEq, Serialize, Deserialize, MapEntities)]
pub enum ProjectileTarget {
    Entity(Entity),
    Position(Position),
    Direction(Vec2),
}

fn setup_lua(lua: &Lua) -> LuaResult<()> {
    let game = lua.table("game")?;

    game.set(
        "register_projectile",
        lua.create_function(|lua, proto: LuaValue| {
            lua.world()
                .resource_mut::<Protos<ProjectileProto>>()
                .insert(proto, lua.current_path())
        })?,
    )?;

    game.set(
        "spawn_projectile",
        lua.create_function(|lua, args: SpawnProjectileArgs| {
            SpawnProjectile(args).apply_from_lua(lua)?;

            Ok(())
        })?,
    )?;

    Ok(())
}

pub struct SpawnProjectile(pub SpawnProjectileArgs);

impl SpawnProjectile {
    fn spawn(self, world: &mut World) -> (ProjectileProto, Entity) {
        let args = self.0;

        let (proto, origin) = world
            .resource::<Protos<ProjectileProto>>()
            .get(&args.proto)
            .unwrap();

        let source_e = world.entity(args.source_unit.entity);
        let team = *source_e.get::<Team>().unwrap();

        let pos = Position(args.position);

        let id = world.spawn((
            Transform::from_translation(pos.into()),
            pos,
            team,
            Model(proto.model.clone().relative(&origin)),
            MapEntity,
            Projectile {
                target: args.target,
                speed: args.speed,
            },
            StateScoped(GameState::InGame),
            ServerReplicate::default(),
        )).id();

        (proto, id)
    }

    fn apply_from_lua(self, lua: &Lua) -> LuaResult<Entity> {
        let mut world = lua.world();

        let (proto, id) = self.spawn(&mut world);

        if let Some(on_spawn) = proto.on_spawn {
            drop(world);
            on_spawn.call::<()>(UnitProxy { entity: id })?;
        }

        Ok(id)
    }
}

impl Command<Entity> for SpawnProjectile {
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


pub struct SpawnProjectileArgs {
    pub proto: String,
    pub position: Vec2,
    pub source_unit: UnitProxy,
    pub target: ProjectileTarget,
    pub speed: f32,
}

from_into_lua_table!(struct SpawnProjectileArgs {
    proto: String,
    position: {W} Vec2,
    source_unit: UnitProxy,
    target: ProjectileTarget,
    speed: f32,
});

impl FromLua for ProjectileTarget {
    fn from_lua(value: LuaValue, lua: &Lua) -> LuaResult<Self> {
        if let Some(userdata) = value.as_userdata()
            && userdata.is::<UnitProxy>()
        {
            Ok(Self::Entity(userdata.borrow::<UnitProxy>()?.entity))
        } else if let Some(table) = value.as_table() {
            let is_dir = table.get::<bool>("is_dir").unwrap_or(false);
            let vec2 = W::<Vec2>::from_lua(value, lua)?;
            if is_dir {
                Ok(ProjectileTarget::Direction(vec2.0))
            } else {
                Ok(ProjectileTarget::Position(Position(vec2.0)))
            }
        } else {
            Err(LuaError::external(anyhow::anyhow!(
                "Expected entity, position or direction"
            )))
        }
    }
}

impl IntoLua for ProjectileTarget {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        match self {
            ProjectileTarget::Entity(entity) => UnitProxy { entity }.into_lua(lua),
            ProjectileTarget::Position(position) => {
                let table = lua.create_table_from([("x", position.x), ("y", position.y)])?;
                table.set("is_dir", true)?;
                Ok(LuaValue::Table(table))
            }
            ProjectileTarget::Direction(vec2) => W(vec2).into_lua(lua),
        }
    }
}

#[derive(PartialEq)]
pub struct ProjectileProto {
    pub id: String,
    pub name: String,
    pub model: AssetPath<'static>,
    pub on_spawn: Option<LuaFunction>,
    pub on_reach_target: Option<LuaFunction>,
    pub on_collide: Option<LuaFunction>,
    pub on_destroy: Option<LuaFunction>,
}

proto! {
    pub struct ProjectileProto {
        pub id: String,
        pub name: String,
        pub model: {W} AssetPath<'static>,
        pub on_spawn: Option<LuaFunction>,
        pub on_reach_target: Option<LuaFunction>,
        pub on_collide: Option<LuaFunction>,
        pub on_destroy: Option<LuaFunction>,
    }
}

fn move_projectile(q: Query<(&Projectile, &mut Position)>, q2: Query<&Position, Without<Projectile>>, time: Res<Time>) {
    for (proj, mut pos) in q {
        match proj.target {
            ProjectileTarget::Entity(entity) => {
                if let Ok(target_pos) = q2.get(entity) {
                    pos.0 = pos.0.move_towards(target_pos.0, proj.speed * time.delta_secs());
                } else {
                    // Target entity died
                    // TODO: make projectile go to target's last position
                }
            },
            ProjectileTarget::Position(position) => {
                pos.0 = pos.0.move_towards(position.0, proj.speed * time.delta_secs());
            },
            ProjectileTarget::Direction(vec2) => {
                pos.0 += vec2.normalize_or_zero() * time.delta_secs();
            },
        }
    }
}
