use std::{collections::HashMap, sync::Arc};

use bevy::{
    app::TerminalCtrlCHandlerPlugin, diagnostic::DiagnosticsPlugin, log::LogPlugin, prelude::*,
    state::app::StatesPlugin,
};
use clap::Parser;
use game::{
    ingame::network::{PROTOCOL_ID, PrivateKey, ServerOptions},
    network::Sess,
};
use lightyear::prelude::{ConnectToken, generate_key};
use lobby_common::{LobbyToServer, ServerToLobby};
use wtransport::{Endpoint, Identity, ServerConfig};

// #[tokio::main]
fn main() -> AppExit {
    let options = ServerOptions::parse();

    let private_key = generate_key();

    let success = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(async move {
            let addr = if let Some(addr) = options.address {
                addr
            } else {
                public_ip_address::perform_lookup(None).await.unwrap().ip
            };

            // Wait for connection from lobby server
            let server = Endpoint::server(
                ServerConfig::builder()
                    .with_bind_default(54653)
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
                return false;
            };

            let mut tokens = HashMap::new();

            for player in players {
                let token = ConnectToken::build(
                    (addr, options.port),
                    PROTOCOL_ID,
                    player.id.0.as_u64_pair().0,
                    private_key,
                )
                .generate()
                .unwrap();

                let bytes = token.try_into_bytes().unwrap();

                tokens.insert(player.id, bytes.to_vec());
            }

            conn.send(ServerToLobby::PlayerTokens { tokens })
                .await
                .unwrap();

            conn.0.0.closed().await;

            true
        });

    if !success {
        return AppExit::error();
    }

    App::new()
        .insert_resource(options)
        .add_plugins((
            MinimalPlugins,
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
        .insert_resource(PrivateKey(private_key))
        .run()
}
