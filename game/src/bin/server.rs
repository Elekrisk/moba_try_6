use std::{sync::Arc, time::Duration};

use bevy::{
    app::{ScheduleRunnerPlugin, TerminalCtrlCHandlerPlugin},
    diagnostic::DiagnosticsPlugin,
    log::LogPlugin,
    platform::collections::HashMap,
    prelude::*,
    state::app::StatesPlugin,
};
use clap::Parser;
use game::{InGamePlayerInfo, PROTOCOL_ID, Players, PrivateKey, ServerOptions, Sess};
use lightyear::prelude::{ClientId, ConnectToken, generate_key};
use lobby_common::{LobbyToServer, ServerToLobby};
use wtransport::{Endpoint, Identity, ServerConfig};

// #[tokio::main]
fn main() -> AppExit {
    let options = ServerOptions::parse();

    let private_key = generate_key();

    let players = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(async move {
            let public_addr = if let Some(addr) = options.public_address {
                println!("Using addr {}", addr);
                addr
            } else {
                public_ip_address::perform_lookup(None).await.unwrap().ip
            };

            let local_addr = options.local_address;

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
                server.accept().await.await.unwrap().accept().await.unwrap(),
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

            for (player, use_global_addr) in players {
                let client_id = player.id.0.as_u64_pair().0;
                let token = ConnectToken::build(
                    (
                        if use_global_addr {
                            public_addr
                        } else {
                            local_addr
                        },
                        options.external_port,
                    ),
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
                        client_id: ClientId::Netcode(client_id),
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
        });

    let Some(players) = players else {
        return AppExit::error();
    };

    App::new()
        .insert_resource(options)
        .insert_resource(players)
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
            TransformPlugin::default(),
            DiagnosticsPlugin::default(),
            TerminalCtrlCHandlerPlugin::default(),
            AssetPlugin::default(),
            StatesPlugin::default(),
            game::server,
        ))
        .run()
}
