use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4, SocketAddrV6};

use bevy::{ecs::entity::MapEntities, platform::collections::HashMap, prelude::*};
use engine_common::ChampionId;
use lightyear::prelude::client::*;
use lightyear::{netcode::ConnectToken, prelude::*};
use lobby_common::{PlayerId, Team};
use serde::{Deserialize, Serialize};

use crate::{
    AppExt, GameState,
    ingame::{map::MessageChannel, unit::MyTeam},
    main_ui::lobby_list::MyPlayerId,
};

#[macro_use]
pub mod lua;
pub mod camera;
pub mod loading;
pub mod map;
pub mod navmesh;
pub mod network;
pub mod structure;
pub mod targetable;
pub mod terrain;
pub mod unit;
pub mod vision;

pub fn client(app: &mut App) {
    app.add_plugins((network::client, camera::client, terrain::client))
        .add_observer(on_disconnect);
    common(app);
}
pub fn server(app: &mut App) {
    app.add_plugins(network::server)
        .add_systems(Startup, network::init_server);
    common(app);
}
pub fn common(app: &mut App) {
    app.add_plugins((
        lua::common,
        network::common,
        map::common,
        targetable::common,
        structure::common,
        terrain::common,
        navmesh::common,
        unit::common,
        loading::plugin,
        vision::plugin,
    ));

    app.register_component::<Players>();
    // app.register_resource::<Players>(lightyear::prelude::ChannelDirection::ServerToClient);
    // app.add_systems(Startup, |mut commands: Commands| {
    //     commands
    //         .replicate_resource::<Players, MessageChannel>(lightyear::prelude::NetworkTarget::All);
    // });

    if app.is_client() {
        app.add_systems(
            Update,
            players_added,
        );

        // TODO: Temporary until game menu is implemented
        app.add_systems(
            Update,
            |input: Res<ButtonInput<KeyCode>>, client: Single<Entity, With<Client>>, mut commands: Commands| {
                if input.just_pressed(KeyCode::Escape) {
                    commands.trigger_targets(Disconnect, *client);
                }
            },
        );
    }
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct SystemSets {}

fn players_added(players: Single<&Players, Changed<Players>>, my_id: Res<MyPlayerId>, mut commands: Commands) {
    for player in players.players.values() {
        if player.id == my_id.0 {
            commands.insert_resource(MyTeam(player.team));
        }
    }
}

pub struct ConnectToGameServer(pub ConnectToken);

impl Command for ConnectToGameServer {
    fn apply(self, world: &mut World) {
        // let client::NetConfig::Netcode { auth, .. } = &mut world.resource_mut::<ClientConfig>().net
        // else {
        //     unreachable!()
        // };

        // *auth = Authentication::Token(self.0);

        // world.connect_client();
        let client = world
            .spawn((
                Client::default(),
                NetcodeClient::new(Authentication::Token(self.0), NetcodeConfig::default())
                    .unwrap(),
                ReplicationReceiver::default(),
                LocalAddr(SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0)),
                PeerAddr(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0))),
                UdpIo::default(),
            ))
            .id();
        world.trigger_targets(Connect, client);
        world.commands().set_state(GameState::Loading);
    }
}

fn on_disconnect(trigger: Trigger<OnAdd, Disconnected>, q: Query<&Disconnected>, mut commands: Commands) {
    if let Some(reason) = &q.get(trigger.target()).unwrap().reason {
        info!("Disconnect: {:?}", reason);
    }
    commands.set_state(GameState::NotInGame);
}

#[derive(Component, Clone, PartialEq, Serialize, Deserialize)]
pub struct Players {
    pub players: HashMap<PlayerId, InGamePlayerInfo>,
}

impl MapEntities for Players {
    fn map_entities<E: EntityMapper>(&mut self, entity_mapper: &mut E) {
        for info in self.players.values_mut() {
            info.map_entities(entity_mapper);
        }
    }
}

#[derive(Resource, Clone, PartialEq, Serialize, Deserialize)]
pub struct InGamePlayerInfo {
    pub id: PlayerId,
    pub client_id: PeerId,
    pub name: String,
    pub team: Team,
    pub champion: ChampionId,
    pub controlled_unit: Option<Entity>,
}

impl MapEntities for InGamePlayerInfo {
    fn map_entities<E: EntityMapper>(&mut self, entity_mapper: &mut E) {
        self.controlled_unit = self.controlled_unit.map(|e| entity_mapper.get_mapped(e));
    }
}
