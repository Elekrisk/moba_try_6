use std::net::IpAddr;

use bevy::prelude::*;
use lightyear::{
    client::config::ClientConfig,
    prelude::{
        ClientDisconnectEvent, ConnectToken,
        client::{self, Authentication, ClientCommandsExt},
        server::{self, ServerCommandsExt},
    },
    server::config::ServerConfig,
};

use crate::ClientState;

#[macro_use]
pub mod lua;
pub mod camera;
pub mod hittable;
pub mod map;
pub mod network;
pub mod structure;
pub mod terrain;
pub mod unit;
pub mod navmesh;

pub fn client(app: &mut App) {
    app.add_plugins((network::client, camera::client, terrain::client))
        .add_systems(Update, on_disconnect);
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
        hittable::common,
        structure::common,
        terrain::common,
        navmesh::common,
        unit::common,
    ));
}

pub struct ConnectToGameServer(pub ConnectToken);

impl Command for ConnectToGameServer {
    fn apply(self, world: &mut World) -> () {
        let client::NetConfig::Netcode { auth, .. } = &mut world.resource_mut::<ClientConfig>().net
        else {
            unreachable!()
        };

        *auth = Authentication::Token(self.0);

        world.connect_client();
        world.commands().set_state(ClientState::InGame);
    }
}

fn on_disconnect(mut events: EventReader<ClientDisconnectEvent>, mut commands: Commands) {
    for event in events.read() {
        info!("Disconnect: {event:?}");
        commands.set_state(ClientState::NotInGame);
    }
}
