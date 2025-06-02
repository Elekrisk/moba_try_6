use std::path::PathBuf;

use bevy::{
    asset::AssetLoader,
    ecs::system::RunSystemOnce,
    prelude::*,
};
use bevy_enhanced_input::{
    events::Fired,
    prelude::{Actions, Binding, InputAction, InputContext, InputContextAppExt, JustPress},
};
use engine_common::{MapDef, MapId};
use lightyear::prelude::{
    AppChannelExt, AppComponentExt, AppMessageExt, Channel, ClientConnectionManager, MessageSend,
    ReliableSettings, Replicated, ServerConnectEvent, ServerConnectionManager,
};
use mlua::prelude::*;
use serde::{Deserialize, Serialize};

use crate::AppExt;

use super::{
    lua::{
        AppLuaExt, ExecuteLuaScript, LuaCtx, LuaExt, LuaScript,
        LuaScriptRunningContext, Protos, ScriptCompleted,
    },
    terrain::Terrain,
};

pub fn common(app: &mut App) {
    app.register_asset_loader(MapDefLoader)
        .init_asset::<MapDefAsset>()
        .setup_lua(setup_lua)
        .init_resource::<Protos<MapProto>>();

    app.register_component::<MapEntity>(lightyear::prelude::ChannelDirection::ServerToClient);

    app.register_message::<UnloadMap>(lightyear::prelude::ChannelDirection::Bidirectional);
    app.register_message::<ResetRegistrations>(lightyear::prelude::ChannelDirection::Bidirectional);
    app.register_message::<LoadMap>(lightyear::prelude::ChannelDirection::Bidirectional);
    app.add_channel::<MessageChannel>(lightyear::prelude::ChannelSettings {
        mode: lightyear::prelude::ChannelMode::OrderedReliable(ReliableSettings::default()),
        ..default()
    });

    // Auto spawn map
    if app.is_client() {
        // app.add_systems(OnEnter(ClientState::InGame), spawn_default_map)
        app.add_input_context::<MapControlContext>()
            .add_systems(
                Update,
                (
                    on_unload_map_client.run_if(resource_exists::<ClientConnectionManager>),
                    on_reset_registration_client.run_if(resource_exists::<ClientConnectionManager>),
                    on_load_map_client.run_if(resource_exists::<ClientConnectionManager>),
                )
                    .chain(),
            )
            .add_observer(bind_input)
            .add_observer(
                |_: Trigger<Fired<ReloadMap>>,
                 mut mgr: ResMut<ClientConnectionManager>| {
                    // commands.queue(UnloadMap);
                    // commands.queue(ResetRegistrations);
                    // commands.queue(LoadMap("default".into()));
                    mgr.send_message::<MessageChannel, _>(&UnloadMap).unwrap();
                    mgr.send_message::<MessageChannel, _>(&ResetRegistrations)
                        .unwrap();
                    mgr.send_message::<MessageChannel, _>(&LoadMap("default".into()))
                        .unwrap();
                },
            )
            .add_systems(Startup, |mut commands: Commands| {
                commands.spawn(Actions::<MapControlContext>::default());
            });
    } else {
        app.add_systems(Startup, spawn_default_map).add_systems(
            Update,
            (
                on_client_connect,
                on_unload_map_server,
                on_reset_registration_server,
                on_load_map_server,
            )
                .chain(),
        );
    }
    app.add_systems(
        Update,
        wait_for_map_load.run_if(resource_exists::<CurrentMapHandle>),
    );
}

pub struct MessageChannel;

impl Channel for MessageChannel {
    fn name() -> &'static str {
        "Message Channel"
    }
}

fn on_unload_map_client(
    mut msgs: EventReader<lightyear::prelude::ClientReceiveMessage<UnloadMap>>,
    mut commands: Commands,
) {
    for msg in msgs.read() {
        info!("Received Unload Map");
        commands.queue(msg.message.clone());
    }
}

fn on_reset_registration_client(
    mut msgs: EventReader<lightyear::prelude::ClientReceiveMessage<ResetRegistrations>>,
    mut commands: Commands,
) {
    for msg in msgs.read() {
        info!("Received Reset Registration");
        commands.queue(msg.message.clone());
    }
}

fn on_load_map_client(
    mut msgs: EventReader<lightyear::prelude::ClientReceiveMessage<LoadMap>>,
    mut commands: Commands,
) {
    for msg in msgs.read() {
        info!("Received Load Map");
        commands.queue(msg.message.clone());
    }
}

fn on_unload_map_server(
    mut msgs: EventReader<lightyear::prelude::ServerReceiveMessage<UnloadMap>>,
    mut mgr: ResMut<ServerConnectionManager>,
    mut commands: Commands,
) {
    for msg in msgs.read() {
        mgr.send_message_to_target::<MessageChannel, _>(
            &msg.message,
            lightyear::prelude::NetworkTarget::All,
        )
        .unwrap();
        commands.queue(msg.message.clone());
    }
}

fn on_reset_registration_server(
    mut msgs: EventReader<lightyear::prelude::ServerReceiveMessage<ResetRegistrations>>,
    mut mgr: ResMut<ServerConnectionManager>,
    mut commands: Commands,
) {
    for msg in msgs.read() {
        mgr.send_message_to_target::<MessageChannel, _>(
            &msg.message,
            lightyear::prelude::NetworkTarget::All,
        )
        .unwrap();
        commands.queue(msg.message.clone());
    }
}

fn on_load_map_server(
    mut mgr: ResMut<ServerConnectionManager>,
    mut msgs: EventReader<lightyear::prelude::ServerReceiveMessage<LoadMap>>,
    mut commands: Commands,
) {
    for msg in msgs.read() {
        mgr.send_message_to_target::<MessageChannel, _>(
            &msg.message,
            lightyear::prelude::NetworkTarget::All,
        )
        .unwrap();
        commands.queue(msg.message.clone());
    }
}

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
    fn apply(self, world: &mut World) -> () {
        world.insert_resource(LuaScriptRunningContext::default());
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct LoadMap(String);

impl Command for LoadMap {
    fn apply(self, world: &mut World) -> () {
        let asset_server = world.resource::<AssetServer>();
        let mapdef: Handle<MapDefAsset> = asset_server.load(format!("maps/{}/def.ron", self.0));
        if asset_server.is_loaded(&mapdef) {
            // Map def asset is loaded
            info!("Map def asset is already loaded");

            let mapdef_assets = world.resource::<Assets<MapDefAsset>>();

            let mapdef = mapdef_assets.get(&mapdef).unwrap();

            let handle = mapdef.script.clone();
            let h = handle.clone();
            world.add_observer(
                move |trigger: Trigger<ScriptCompleted>, mut commands: Commands| {
                    if trigger.0 == handle {
                        // Map script has finished registering
                        commands.entity(trigger.observer()).despawn();

                        commands.queue(move |world: &mut World| {
                            let lua = world.resource::<LuaCtx>().0.clone();

                            let (proto, path) =
                                world.resource::<Protos<MapProto>>().get("default").unwrap();

                            if let Some(on_load) = proto.on_load {
                                lua.set_path(path);
                                lua.with_world(world, |_| on_load.call::<()>(())).unwrap();
                            } else {
                                panic!()
                            }
                        });
                    }
                },
            );
            ExecuteLuaScript::new(h).immediately().apply(world);
        }
        world.insert_resource(CurrentMapHandle(mapdef));
    }
}

#[derive(Component, Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize, Reflect)]
pub struct MapEntity;

fn setup_lua(lua: &Lua) -> mlua::Result<()> {
    // Map global

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

#[derive(Resource)]
struct CurrentMapHandle(Handle<MapDefAsset>);

fn spawn_default_map(
    mut commands: Commands,
) {
    let load_map = LoadMap("default".into());
    commands.queue(load_map);
    // mgr.send_message_to_target::<MessageChannel, _>(&load_map, lightyear::prelude::NetworkTarget::All).unwrap();
}

fn on_client_connect(
    mut events: EventReader<ServerConnectEvent>,
    mut mgr: ResMut<ServerConnectionManager>,
) {
    for event in events.read() {
        mgr.send_message::<MessageChannel, _>(event.client_id, &LoadMap("default".into()))
            .unwrap();
    }
}

fn wait_for_map_load(
    mut asset_events: EventReader<AssetEvent<MapDefAsset>>,
    current_map_handle: Res<CurrentMapHandle>,
    mapdef_assets: Res<Assets<MapDefAsset>>,
    mut commands: Commands,
) {
    for event in asset_events.read() {
        match event {
            AssetEvent::LoadedWithDependencies { id } => {
                if current_map_handle.0.id() == *id {
                    let mapdef = mapdef_assets.get(*id).unwrap();
                    let handle = mapdef.script.clone();
                    let h = handle.clone();
                    commands.add_observer(
                        move |trigger: Trigger<ScriptCompleted>, mut commands: Commands| {
                            if trigger.0 == handle {
                                // Map script has finished registering
                                commands.entity(trigger.observer()).despawn();

                                commands.queue(move |world: &mut World| {
                                    let lua = world.resource::<LuaCtx>().0.clone();

                                    let (proto, path) = world
                                        .resource::<Protos<MapProto>>()
                                        .get("default")
                                        .unwrap();

                                    if let Some(on_load) = proto.on_load {
                                        lua.set_path(path);
                                        lua.with_world(world, |_| on_load.call::<()>(())).unwrap();
                                    }
                                });
                            }
                        },
                    );
                    commands.queue(ExecuteLuaScript::new(h).immediately());
                } else {
                }
            }
            _ => {}
        }
    }
}

pub struct SpawnMap(MapDef);

impl Command for SpawnMap {
    fn apply(self, world: &mut World) -> () {
        let lua = world.resource::<LuaCtx>();
        lua.load(self.0.script).exec().unwrap();
    }
}

struct MapDefLoader;

#[derive(Reflect, Asset)]
struct MapDefAsset {
    id: MapId,
    name: String,
    script: Handle<LuaScript>,
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
                info!("Path: {}", cur_path.path().display());
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
        &["ron"]
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
    info!("BINDING RELOAD MAP");
    actions
        .bind::<ReloadMap>()
        .to(KeyCode::F10)
        .with_conditions(JustPress::default())
        ;
}
