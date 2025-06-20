use std::{net::IpAddr, sync::Arc, time::Duration};

use bevy::{
    app::{MainScheduleOrder, ScheduleRunnerPlugin, TerminalCtrlCHandlerPlugin},
    asset::uuid::Uuid,
    diagnostic::DiagnosticsPlugin,
    ecs::schedule::ScheduleLabel,
    log::LogPlugin,
    platform::collections::HashMap,
    prelude::*,
    state::app::StatesPlugin,
};
use clap::Parser;
use engine_common::ChampionId;
use game::{
    InGamePlayerInfo, PROTOCOL_ID, Players, PrivateKey, ServerFixedUpdateDuration, ServerOptions,
    Sess,
};
use lightyear::{
    netcode::{ConnectToken, generate_key},
    prelude::{NetworkTarget, PeerId, Replicate},
};
use lobby_common::{LobbyToServer, PlayerId, ServerToLobby, Team};
use wtransport::{Endpoint, Identity, ServerConfig};

// #[tokio::main]
fn main() -> AppExit {
    let options = ServerOptions::parse();

    let private_key = if options.direct_connect {
        [0; 32]
    } else {
        generate_key()
    };

    let players = if !options.direct_connect {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async move {
                // Wait for connection from lobby server
                let server = Endpoint::server(
                    ServerConfig::builder()
                        .with_bind_default(options.internal_port)
                        .with_identity(
                            Identity::self_signed([
                                "localhost",
                                "127.0.0.1",
                                "::1",
                                "moba.elekrisk.com",
                            ])
                            .unwrap(),
                        )
                        .build(),
                )
                .unwrap();

                let conn = Sess(Arc::new(xwt_wtransport::Connection(
                    tokio::time::timeout(Duration::from_secs(5), async {
                        server.accept().await.await.unwrap().accept().await.unwrap()
                    })
                    .await
                    .unwrap(),
                )));

                #[allow(irrefutable_let_patterns)]
                let LobbyToServer::Handshake {
                    settings: _,
                    players,
                } = conn.recv().await.unwrap()
                else {
                    return None;
                };

                let mut tokens = std::collections::HashMap::new();

                let mut player_infos = HashMap::new();

                for player in players {
                    let client_id = player.id.0.as_u64_pair().0;
                    let ip_addr: IpAddr = match (player.is_ipv4, player.is_local) {
                        (true, true) => options.local_address_ipv4.into(),
                        (true, false) => options.public_address_ipv4.into(),
                        (false, _) => options.address_ipv6.into(),
                    };
                    println!("For player {}, use address {}", player.name, ip_addr);
                    let token = ConnectToken::build(
                        (ip_addr, options.external_port),
                        PROTOCOL_ID,
                        client_id,
                        private_key,
                    )
                    .generate()
                    .unwrap();

                    player_infos.insert(
                        player.id,
                        InGamePlayerInfo {
                            id: player.id,
                            name: player.name,
                            client_id: PeerId::Netcode(client_id),
                            team: player.team,
                            champion: player.champ,
                            controlled_unit: None,
                        },
                    );

                    let bytes = token.try_into_bytes().unwrap();

                    tokens.insert(player.id, bytes.to_vec());
                }

                conn.send(ServerToLobby::PlayerTokens { tokens })
                    .await
                    .unwrap();

                conn.0.0.closed().await;

                Some(Players {
                    players: player_infos,
                })
            })
    } else {
        Some(Players {
            players: HashMap::from_iter([(
                PlayerId(Uuid::nil()),
                InGamePlayerInfo {
                    id: PlayerId(Uuid::nil()),
                    client_id: PeerId::Netcode(0),
                    name: "Guest".into(),
                    team: Team(0),
                    champion: ChampionId("example_champion".into()),
                    controlled_unit: None,
                },
            )]),
        })
    };

    let Some(players) = players else {
        return AppExit::error();
    };

    #[derive(ScheduleLabel, Debug, Clone, Copy, PartialEq, Eq, Hash)]
    struct PrePreStartup;

    let mut app = App::new();
    app.insert_resource(options)
        .insert_resource(PrivateKey(private_key))
        .add_plugins((
            MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(
                // Run 60 times per second.
                Duration::from_secs_f64(1.0 / 60.0),
            )),
            LogPlugin {
                // filter: "lightyear=debug".into(),
                // level: bevy::log::Level::DEBUG,
                ..default()
            },
            TransformPlugin,
            DiagnosticsPlugin,
            TerminalCtrlCHandlerPlugin,
            AssetPlugin::default(),
            StatesPlugin,
            game::server,
        ));

    let mut order = app.world_mut().resource_mut::<MainScheduleOrder>();
    order.insert_startup_before(StateTransition, PrePreStartup);

    app.add_systems(PrePreStartup, move |mut commands: Commands| {
        info!("Spawning players");
        commands.spawn((players.clone(), Replicate::to_clients(NetworkTarget::All)));
    })
    .insert_resource(Timing(std::time::Instant::now()))
    .add_systems(FixedFirst, |mut timing: ResMut<Timing>| {
        timing.0 = std::time::Instant::now();
    })
    // .add_systems(
    //     FixedLast,
    //     |timing: Res<Timing>, mut fixed_update: ResMut<ServerFixedUpdateDuration>| {
    //         let time = std::time::Instant::now()
    //             .duration_since(timing.0)
    //             .as_secs_f32();
    //         fixed_update.0 = time;
    //     },
    // )
    .run()
}

#[derive(Resource)]
struct Timing(std::time::Instant);
