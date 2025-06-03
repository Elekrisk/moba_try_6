#![feature(decl_macro)]
#![feature(mpmc_channel)]
#![feature(iter_array_chunks)]
#![feature(type_alias_impl_trait)]
#![feature(anonymous_lifetime_in_impl_trait)]
#![feature(iter_intersperse)]
#![feature(impl_trait_in_assoc_type)]
#![feature(debug_closure_helpers)]
#![feature(array_windows)]

use std::{fmt::Display, path::PathBuf};

use bevy::{
    asset::AssetLoader, platform::collections::HashMap, prelude::*
};

mod r#async;
mod main_ui;
mod network;
mod new_ui;
mod ui;
mod ingame;

pub use ingame::{Players, InGamePlayerInfo, network::{PROTOCOL_ID, PrivateKey, ServerOptions}};
pub use network::Sess;

use engine_common::{ChampList, ChampionDef, ChampionId};
use lightyear::prelude::ServerConnectionManager;
pub use network::LobbySender;

pub fn client(app: &mut App) {
    common(app);

    app.add_plugins((ui::client, new_ui::client, main_ui::client, network::client, ingame::client));
    app.add_systems(Startup, client_setup)
        .add_systems(OnEnter(ClientState::InGame), || info!("Entered InGame state"));

    app.insert_state(if app.world().resource::<Options>().immediately_ingame {
        ClientState::InGame
    } else {
        ClientState::NotInGame
    });
}

pub fn server(app: &mut App) {
    common(app);

    app.add_plugins(ingame::server);

    app.add_systems(Update, |ps: Option<Res<ServerConnectionManager>>, mut exit: EventWriter<AppExit>, time: Res<Time>| {
        if let Some(server) = ps {
            if server.connected_clients().next().is_none() && time.elapsed_secs() > 15.0 {
                exit.write(AppExit::Success);
            }
        }
    });
}

fn common(app: &mut App) {
    app.add_plugins(r#async::common);
    app.register_asset_loader(ChampionDefLoader)
        .register_asset_loader(ChampDefsLoader)
        .init_asset::<ChampionDef>()
        .init_asset::<ChampDefsAsset>()
        .add_systems(Update, wait_for_list_load)
        .add_systems(Startup, load_champ_defs);
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, States)]
#[states(scoped_entities)]
pub enum ClientState {
    #[default]
    NotInGame,
    InGame,
}

fn client_setup(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection::default()),
        Transform::from_xyz(0.0, 55.0, 55.0).looking_at(Vec3::ZERO, Vec3::Y),
        // Transform::from_xyz(0.0, 55.0, 0.0).looking_at(Vec3::ZERO, Vec3::new(-1.0, 0.0, -1.0).normalize())
    ));
    commands.spawn((DirectionalLight {
        ..default()
    }, Transform::from_xyz(15.0, 15.0, 15.0).looking_at(Vec3::ZERO, Vec3::Y)));
}

#[derive(Resource, clap::Parser)]
pub struct Options {
    #[arg(long)]
    connect: bool,
    #[arg(long, default_value_t = LobbyMode::None)]
    lobby_mode: LobbyMode,
    #[arg(long)]
    auto_start: bool,
    #[arg(long)]
    pub log_file: Option<PathBuf>,
    #[arg(long)]
    immediately_ingame: bool,
}

#[derive(Clone, Default, clap::ValueEnum)]
pub enum LobbyMode {
    #[default]
    None,
    AutoCreate,
    AutoJoinFirst,
}

impl Display for LobbyMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::None => "none",
                Self::AutoCreate => "auto-create",
                Self::AutoJoinFirst => "auto-join-first",
            }
        )
    }
}

#[derive(Reflect, Asset)]
struct ChampDefsAsset {
    map: HashMap<String, Handle<ChampionDef>>,
}

#[derive(Resource)]
struct ChampDefsHandle(#[allow(dead_code)] Handle<ChampDefsAsset>);

#[derive(Debug, Resource)]
struct ChampDefs {
    map: HashMap<ChampionId, ChampionDef>,
}

fn load_champ_defs(server: Res<AssetServer>, mut commands: Commands) {
    commands.insert_resource(ChampDefsHandle(
        server.load::<ChampDefsAsset>("champs/champ_list.ron"),
    ));
}

fn wait_for_list_load(
    mut event: EventReader<AssetEvent<ChampDefsAsset>>,
    assets: Res<Assets<ChampDefsAsset>>,
    defs: Res<Assets<ChampionDef>>,
    mut commands: Commands,
) {
    for e in event.read() {
        match e {
            AssetEvent::LoadedWithDependencies { id } => {
                let asset = assets.get(*id).unwrap();
                commands.insert_resource(ChampDefs {
                    map: asset
                        .map
                        .values()
                        .map(|handle| {
                            let def = defs.get(handle).unwrap();
                            (def.id, def.clone())
                        })
                        .collect(),
                });
            }
            _ => {}
        }
    }
}

struct ChampionDefLoader;

impl AssetLoader for ChampionDefLoader {
    type Asset = ChampionDef;

    type Settings = ();

    type Error = anyhow::Error;

    fn load(
        &self,
        reader: &mut dyn bevy::asset::io::Reader,
        _settings: &Self::Settings,
        _load_context: &mut bevy::asset::LoadContext,
    ) -> impl bevy::tasks::ConditionalSendFuture<Output = std::result::Result<Self::Asset, Self::Error>>
    {
        async move {
            let mut buf = vec![];
            reader.read_to_end(&mut buf).await?;
            let asset = ron::de::from_bytes(&buf)?;

            Ok(asset)
        }
    }

    fn extensions(&self) -> &[&str] {
        &["ron"]
    }
}

struct ChampDefsLoader;

impl AssetLoader for ChampDefsLoader {
    type Asset = ChampDefsAsset;

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
            let list: ChampList = ron::de::from_bytes(&buf)?;

            let mut asset = ChampDefsAsset {
                map: HashMap::new(),
            };

            for champ in list.0 {
                let handle = load_context.load(format!("champs/{champ}/def.ron"));
                asset.map.insert(champ, handle);
            }

            Ok(asset)
        }
    }

    fn extensions(&self) -> &[&str] {
        &["ron"]
    }
}

pub trait AppExt {
    fn is_server(&self) -> bool;
    fn is_client(&self) -> bool {
        !self.is_server()
    }
}

impl AppExt for App {
    fn is_server(&self) -> bool {
        self.world().contains_resource::<ServerOptions>()
    }
}
