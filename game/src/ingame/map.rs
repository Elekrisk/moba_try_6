use std::path::PathBuf;

use bevy::{
    asset::{AssetLoader, AssetPath}, ecs::system::RunSystemOnce, math::VectorSpace, pbr::FogVolume, prelude::*
};
use bevy_enhanced_input::{
    events::Fired,
    prelude::{Actions, Binding, InputAction, InputContext, InputContextAppExt, Press},
};
use engine_common::{MapDef, MapId};
use lightyear::prelude::{
    AppChannelExt, AppComponentExt, Channel, ReliableSettings, Replicated, ServerReplicate,
    server::SyncTarget,
};
use mlua::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    ingame::{
        structure::Model,
        targetable::Health,
        unit::{ControlledByClient, SpawnUnit, SpawnUnitArgs, Unit},
        
    }, AppExt, Players, ServerOptions
};

use super::{
    lua::{AppLuaExt, LuaCtx, LuaExt, LuaScript, Protos},
    terrain::Terrain,
};

pub fn common(app: &mut App) {
    app.register_asset_loader(MapDefLoader)
        .init_asset::<MapDefAsset>()
        .setup_lua(setup_lua)
        .init_resource::<Protos<MapProto>>();

    app.register_component::<MapEntity>(lightyear::prelude::ChannelDirection::ServerToClient);

    app.add_channel::<MessageChannel>(lightyear::prelude::ChannelSettings {
        mode: lightyear::prelude::ChannelMode::OrderedReliable(ReliableSettings::default()),
        ..default()
    });

    // Auto spawn map
    if app.is_client() {
        // app.add_systems(OnEnter(ClientState::InGame), spawn_default_map)
        app.add_input_context::<MapControlContext>()
            .add_observer(bind_input)
            .add_observer(
                |_: Trigger<Fired<ReloadMap>>,
                //  mut mgr: ResMut<ClientConnectionManager>
                 | {
                    // commands.queue(UnloadMap);
                    // commands.queue(ResetRegistrations);
                    // commands.queue(LoadMap("default".into()));
                    // mgr.send_message::<MessageChannel, _>(&UnloadMap).unwrap();
                    // mgr.send_message::<MessageChannel, _>(&ResetRegistrations)
                    //     .unwrap();
                    // mgr.send_message::<MessageChannel, _>(&LoadMap("default".into()))
                    //     .unwrap();
                },
            )
            .add_systems(Startup, |mut commands: Commands| {
                commands.spawn(Actions::<MapControlContext>::default());
            });
    } else {
    }
}

pub struct MessageChannel;

impl Channel for MessageChannel {
    fn name() -> &'static str {
        "Message Channel"
    }
}

#[derive(PartialEq)]
pub struct MapProto {
    pub id: String,
    pub name: String,
    pub on_load: Option<LuaFunction>,
    pub script_path: Option<PathBuf>,
}

proto! {
    struct MapProto {
        id: String,
        name: String,
        on_load: Option<LuaFunction>,
        script_path: Option<PathBuf>,
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct UnloadMap;

impl Command for UnloadMap {
    fn apply(self, world: &mut World) -> () {
        world
            .run_system_once(
                |q: Query<Entity, (With<MapEntity>, Without<Replicated>)>,
                 mut commands: Commands| {
                    for e in &q {
                        commands.entity(e).despawn();
                    }
                },
            )
            .unwrap();
        world.resource_mut::<Terrain>().objects.clear();
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ResetRegistrations;

impl Command for ResetRegistrations {
    fn apply(self, _world: &mut World) -> () {
        // world.insert_resource(LuaScriptRunningContext::default());
    }
}

/// Loads the specified map.
///
/// The map definition needs to have already been loaded from disk,
/// and it's registration script needs to have been ran.
#[derive(Clone)]
pub struct LoadMap(pub MapId);

impl Command for LoadMap {
    fn apply(self, world: &mut World) -> () {
        let lua = world.resource::<LuaCtx>().0.clone();

        let (proto, path) = world.resource::<Protos<MapProto>>().get(&self.0.0).unwrap();

        if let Some(on_load) = proto.on_load {
            lua.set_path(path);
            match lua.with_world(world, |_| on_load.call::<()>(())) {
                Ok(()) => {}
                Err(e) => error!("Lua error: {e}"),
            };
        } else {
            warn!("Map has no on_load!");
        }

        // On the server, we want to spawn one unit per player
        info!("On the server, load player units");

        if world.contains_resource::<ServerOptions>() {
            info!("Load player units");
            world.commands().queue(SpawnPlayerUnits {})
        } else {
            // info!("Spawn fog of war");
            // let fog_image = FogOfWarTexture::new(vec2(50.0, 50.0), world.resource::<AssetServer>());
            // world.spawn((
            //     {
            //         let mut trans =
            //             Transform::from_scale(vec3(100.0, 100.0, 2.0)).with_translation(Vec3::Y);
            //         trans.rotate_local_x(90.0f32.to_radians());
            //         trans
            //     },
            //     FogVolume {
            //         density_factor: 1.0,
            //         scattering: 1.0,
            //         density_texture: Some(fog_image.0.clone()),
            //         ..default()
            //     },
            // ));
            // world.insert_resource(fog_image);
        }
    }
}

pub struct SpawnPlayerUnits {}

impl Command for SpawnPlayerUnits {
    fn apply(self, world: &mut World) -> () {
        // We need to get each player
        world.resource_scope(|world, players: Mut<Players>| {
            for player in players.players.values() {
                let id = SpawnUnit(SpawnUnitArgs {
                    proto: player.champion.0.clone(),
                    position: Vec2::ZERO,
                    team: player.team,
                    data: super::unit::effect::CustomData::Nil,
                }).apply(world);
                world.entity_mut(id).insert(ControlledByClient(player.client_id));
            }
        });
    }
}

#[derive(Component, Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize, Reflect)]
pub struct MapEntity;

fn setup_lua(lua: &Lua) -> mlua::Result<()> {
    let game = lua.table("game")?;

    game.set(
        "register_map",
        lua.create_function(|lua: &Lua, proto: LuaValue| {
            // Make sure proto is of correct type
            lua.world()
                .resource_mut::<Protos<MapProto>>()
                .insert(proto, lua.current_path())
        })?,
    )?;

    game.set(
        "spawn_floor_plane",
        lua.create_function(|lua: &Lua, args: FloorPlaneArgs| {
            if lua.is_server() {
                return Ok(());
            }

            let mut world = lua.world();

            // Create a mesh
            let mesh = Plane3d::new(Vec3::Y, args.dimensions / 2.0).mesh().build();
            let mesh = world.resource::<AssetServer>().add(mesh);
            let image = world.resource::<AssetServer>().load(lua.path(&args.image));
            let mat = world.resource::<AssetServer>().add(StandardMaterial {
                base_color_texture: Some(image),
                ..default()
            });

            world.spawn((
                Mesh3d(mesh),
                MeshMaterial3d(mat),
                Transform::from_xyz(0.0, 0.0, 0.0),
                MapEntity,
            ));

            Ok(())
        })?,
    )?;

    Ok(())
}

from_into_lua_table!(struct FloorPlaneArgs {
    dimensions: {W} Vec2,
    image: PathBuf,
});

#[derive(Debug, Clone, Reflect)]
pub struct FloorPlaneArgs {
    dimensions: Vec2,
    image: PathBuf,
}
struct MapDefLoader;

#[derive(Reflect, Asset)]
pub struct MapDefAsset {
    pub id: MapId,
    pub name: String,
    pub script: Handle<LuaScript>,
}

impl AssetLoader for MapDefLoader {
    type Asset = MapDefAsset;

    type Settings = ();

    type Error = anyhow::Error;

    fn load(
        &self,
        reader: &mut dyn bevy::asset::io::Reader,
        _settings: &Self::Settings,
        load_context: &mut bevy::asset::LoadContext,
    ) -> impl bevy::tasks::ConditionalSendFuture<Output = std::result::Result<Self::Asset, Self::Error>>
    {
        async move {
            let mut buf = vec![];
            reader.read_to_end(&mut buf).await?;
            let mapdef: MapDef = ron::de::from_bytes(&buf)?;

            let cur_path = load_context.asset_path();

            let path = if mapdef.script.starts_with(".") || mapdef.script.starts_with("..") {
                cur_path.path().parent().unwrap().join(&mapdef.script)
            } else {
                mapdef.script
            };

            Ok(MapDefAsset {
                id: mapdef.id,
                name: mapdef.name,
                script: load_context.load(path),
            })
        }
    }

    fn extensions(&self) -> &[&str] {
        &["map.ron"]
    }
}

#[derive(InputContext)]
pub struct MapControlContext;

#[derive(Debug, InputAction)]
#[input_action(output = bool)]
pub struct ReloadMap;

fn bind_input(
    trigger: Trigger<Binding<MapControlContext>>,
    mut actions: Query<&mut Actions<MapControlContext>>,
) {
    let mut actions = actions.get_mut(trigger.target()).unwrap();
    actions
        .bind::<ReloadMap>()
        .to(KeyCode::F10)
        .with_conditions(Press::default());
}
