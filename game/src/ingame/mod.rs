use std::net::IpAddr;

use bevy::prelude::*;
use lightyear::{
    client::config::ClientConfig,
    prelude::{
        client::{self, Authentication, ClientCommandsExt}, server::{self, ServerCommandsExt}, ConnectToken
    },
    server::config::ServerConfig,
};

use crate::ClientState;

pub mod network;
pub mod lua;
pub mod map;

pub fn client(app: &mut App) {
    common(app);

    app.add_plugins(network::client);
}
pub fn server(app: &mut App) {
    common(app);

    app.add_plugins(network::server)
        .add_systems(Startup, network::init_server);
}
pub fn common(_app: &mut App) {}

pub struct ConnectToGameServer(pub ConnectToken);

impl Command for ConnectToGameServer {
    fn apply(self, world: &mut World) -> () {
        let client::NetConfig::Netcode { auth, .. } = &mut world.resource_mut::<ClientConfig>().net
        else {
            unreachable!()
        };

        *auth = Authentication::Token(self.0);

        println!("Connecting...");
        world.connect_client();
        world.commands().set_state(ClientState::InGame);
    }
}
