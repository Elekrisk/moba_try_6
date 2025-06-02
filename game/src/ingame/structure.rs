
use bevy::{asset::AssetPath, prelude::*};
use lightyear::prelude::{AppComponentExt, ChannelDirection, ServerReplicate};
use lobby_common::Team;
use mlua::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{ingame::{map::MapEntity, navmesh::TerrainData}, AppExt};

use super::{
    hittable::Health,
    lua::{AppLuaExt, AssetPathExt, LuaExt, Protos},
};

pub fn common(app: &mut App) {
    app.init_resource::<Protos<StructProto>>()
        .setup_lua(setup_lua);

    if app.is_client() {
        app.add_observer(on_insert_model);
    }

    app.register_component::<Model>(ChannelDirection::ServerToClient);
}

pub struct StructProto {
    pub id: String,
    pub name: String,
    pub model: AssetPath<'static>,
    pub health: f32,
    pub radius: f32,
    pub on_spawn: Option<LuaFunction>,
    pub on_destroy: Option<LuaFunction>,
    pub custom_data: Option<LuaValue>,
}

proto!(
    struct StructProto {
        id: String,
        name: String,
        model: {W} AssetPath<'static>,
        health: f32,
        radius: f32,
        on_spawn: Option<LuaFunction>,
        on_destroy: Option<LuaFunction>,
        custom_data: Option<LuaValue>,
    }
);

fn setup_lua(lua: &Lua) -> LuaResult<()> {
    let game = lua.table("game")?;

    game.set(
        "register_structure",
        lua.create_function(|lua: &Lua, proto: LuaValue| {
            lua.world()
                .resource_mut::<Protos<StructProto>>()
                .insert(proto, lua.current_path())
        })?,
    )?;

    game.set(
        "spawn_structure",
        lua.create_function(|lua: &Lua, args: SpawnStructureArgs| {
            let mut world = lua.world();
            
            let (proto, path) = world.resource::<Protos<StructProto>>().get(&args.proto)?;

            // let asset_server = world.resource::<AssetServer>().clone();

            // let path_rel = lua.path_rel(&proto.model.path(), path);
            // let mut path = AssetPath::from(path_rel);
            // if let Some(label) = proto.model.label() {
            //     path = path.with_label(label.to_string());
            // }

            let path = proto.model.relative(&path);

            if lua.is_server() {
                // The server should own the entity
                info!("Spawning structure! Should replicate to client");

                // Calculate terrain data
                let perimeter = Circle::new(proto.radius).perimeter();
                // Vertex step of 0.1, at least 4 vertices
                let vertices = ((perimeter / 0.1) as usize).clamp(4, 32);
                let vertex_step = perimeter / vertices as f32;

                let point_on_circle = |length: f32| {
                    let angle = (length / perimeter) * std::f32::consts::PI * 2.0;
                    vec2(angle.cos(), angle.sin())
                };

                let vertices = (0..vertices)
                    .map(|i| point_on_circle(vertex_step * i as f32) * proto.radius)
                    .collect::<Vec<_>>();

                info!("Spawning with {} points", vertices.len());

                world.spawn((
                    Transform::from_xyz(args.position.x, 0.0, args.position.y),
                    Team(args.team),
                    Health(proto.health),
                    Model(path),
                    TerrainData {
                        vertices,
                    },
                    MapEntity,
                    ServerReplicate::default(),
                ));
            }

            if lua.is_client() {
                // let model = asset_server.load(path);

                // entity.insert(SceneRoot(model));
            }

            Ok(())
        })?,
    )?;

    Ok(())
}

pub struct SpawnStructureArgs {
    proto: String,
    team: usize,
    position: Vec2,
}

from_into_lua_table!(
    struct SpawnStructureArgs {
        proto: String,
        team: usize,
        position: {W} Vec2,
    }
);

#[derive(Component, Debug, PartialEq, Serialize, Deserialize)]
pub struct Model(pub AssetPath<'static>);

fn on_insert_model(
    trigger: Trigger<OnInsert, Model>,
    query: Query<&Model>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    let Ok(model) = query.get(trigger.target()) else {
        error!(
            "Failed to get model for newly added model (entity {})",
            trigger.target()
        );
        return;
    };

    info!("Loading {}", model.0);
    let handle = asset_server.load(&model.0);

    commands.entity(trigger.target()).insert(SceneRoot(handle));
}
