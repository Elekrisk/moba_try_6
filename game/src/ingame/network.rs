use std::{
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
    time::Duration,
};

use bevy::prelude::*;
use lightyear::{
    netcode::{NetcodeServer, PRIVATE_KEY_BYTES}, prelude::{client::ClientPlugins, server::{NetcodeConfig, ServerPlugins, ServerUdpIo, Start}, *}
};
use lobby_common::Team;

use crate::AppExt;

// fn shared_config() -> SharedConfig {
//     SharedConfig {
//         server_replication_send_interval: Duration::ZERO,
//         client_replication_send_interval: Duration::ZERO,
//         tick: TickConfig {
//             tick_duration: Duration::from_secs_f32(1.0 / 20.0),
//         },
//     }
// }

pub const PROTOCOL_ID: u64 = 2478926748297;

pub fn client(app: &mut App) {
    // let config = ClientConfig {
    //     shared: shared_config(),
    //     net: client::NetConfig::Netcode {
    //         auth: client::Authentication::None,
    //         config: default(),
    //         io: client::IoConfig::from_transport(client::ClientTransport::UdpSocket(
    //             SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0)),
    //         )),
    //     },
    //     ..default()
    // };

    // app.add_plugins(ClientPlugins::new(config));

    app.add_plugins(ClientPlugins {
        tick_duration: Duration::from_secs_f64(1.0 / 20.0)
    });
}

pub fn server(app: &mut App) {
    // let config = ServerConfig {
    //     shared: shared_config(),
    //     net: vec![server::NetConfig::Netcode {
    //         config: server::NetcodeConfig::default().with_protocol_id(PROTOCOL_ID),
    //         io: server::IoConfig::from_transport(server::ServerTransport::UdpSocket(
    //             SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, 54655, 0, 0)),
    //         )),
    //     }],
    //     ..default()
    // };

    app.add_plugins(ServerPlugins {
        tick_duration: Duration::from_secs_f64(1.0 / 20.0)
    });
}

pub fn common(app: &mut App) {
    if app.is_client() {
        // app.add_plugins(client::VisualInterpolationPlugin::<Transform>::default());
    }
    // app.register_component::<Transform>(ChannelDirection::ServerToClient)
    //     .add_interpolation(ComponentSyncMode::Full)
    //     .add_interpolation_fn(|from, to, t| {
    //         let trans = from.translation.lerp(to.translation, t);
    //         let rot = from.rotation.slerp(to.rotation, t);
    //         let scale = from.scale.lerp(to.scale, t);

    //         Transform::default()
    //             .with_translation(trans)
    //             .with_rotation(rot)
    //             .with_scale(scale)
    //     });
    app.register_component::<Team>();
}

#[derive(Resource, clap::Parser)]
pub struct ServerOptions {
    pub public_address_ipv4: Ipv4Addr,
    pub local_address_ipv4: Ipv4Addr,
    pub address_ipv6: Ipv6Addr,
    pub internal_port: u16,
    pub external_port: u16,
    #[arg(long)]
    pub direct_connect: bool,
}

#[derive(Resource)]
pub struct PrivateKey(pub [u8; PRIVATE_KEY_BYTES]);

pub fn init_server(
    options: Res<ServerOptions>,
    key: Res<PrivateKey>,
    // mut config: ResMut<ServerConfig>,
    mut commands: Commands,
) {
    // let server::NetConfig::Netcode { config, io } = &mut config.net[0];
    // config.private_key = key.0;
    // let server::ServerTransport::UdpSocket(port) = &mut io.transport else {
    //     unreachable!()
    // };
    // port.set_port(options.external_port);
    // commands.start_server();

    let server = commands.spawn((
        NetcodeServer::new(NetcodeConfig::default().with_key(key.0).with_protocol_id(PROTOCOL_ID)),
        LocalAddr(SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, options.external_port, 0, 0))),
        ServerUdpIo::default()
    )).id();

    commands.trigger_targets(Start, server);
}
