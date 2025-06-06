use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4},
    time::Duration,
};

use bevy::prelude::*;
use lightyear::{
    client::{config::ClientConfig, plugin::ClientPlugins},
    connection::netcode::PRIVATE_KEY_BYTES,
    prelude::{server::ServerCommandsExt as _, *},
    server::{config::ServerConfig, plugin::ServerPlugins},
};
use lobby_common::Team;

use crate::AppExt;

fn shared_config() -> SharedConfig {
    SharedConfig {
        server_replication_send_interval: Duration::ZERO,
        client_replication_send_interval: Duration::ZERO,
        tick: TickConfig {
            tick_duration: Duration::from_secs_f32(1.0 / 20.0),
        },
    }
}

pub const PROTOCOL_ID: u64 = 2478926748297;

pub fn client(app: &mut App) {
    let config = ClientConfig {
        shared: shared_config(),
        net: client::NetConfig::Netcode {
            auth: client::Authentication::None,
            config: default(),
            io: client::IoConfig::from_transport(client::ClientTransport::UdpSocket(
                SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0)),
            )),
        },
        ..default()
    };

    app.add_plugins(ClientPlugins::new(config));
}

pub fn server(app: &mut App) {
    let config = ServerConfig {
        shared: shared_config(),
        net: vec![server::NetConfig::Netcode {
            config: server::NetcodeConfig::default().with_protocol_id(PROTOCOL_ID),
            io: server::IoConfig::from_transport(server::ServerTransport::UdpSocket(
                SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 54655)),
            )),
        }],
        ..default()
    };

    app.add_plugins(ServerPlugins::new(config));
}

pub fn common(app: &mut App) {
    if app.is_client() {
        // app.add_plugins(
        //     VisualInterpolationPlugin::<Transform>::default(),
        // );
    }
    app.register_component::<Transform>(ChannelDirection::ServerToClient)
        // .add_custom_interpolation(client::ComponentSyncMode::Full)
        ;
    app.register_component::<Team>(ChannelDirection::ServerToClient);
}

#[derive(Resource, clap::Parser)]
pub struct ServerOptions {
    #[arg(long)]
    pub public_address: Option<IpAddr>,
    pub local_address: IpAddr,
    pub internal_port: u16,
    pub external_port: u16,
}

#[derive(Resource)]
pub struct PrivateKey(pub [u8; PRIVATE_KEY_BYTES]);

pub fn init_server(
    options: Res<ServerOptions>,
    key: Res<PrivateKey>,
    mut config: ResMut<ServerConfig>,
    mut commands: Commands,
) {
    let server::NetConfig::Netcode { config, io } = &mut config.net[0];
    config.private_key = key.0;
    let server::ServerTransport::UdpSocket(port) = &mut io.transport else {
        unreachable!()
    };
    port.set_port(options.external_port);
    commands.start_server();
}
