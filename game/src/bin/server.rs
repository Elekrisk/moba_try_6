use std::{collections::HashMap, sync::Arc};

use bevy::{
    app::TerminalCtrlCHandlerPlugin, diagnostic::DiagnosticsPlugin, log::LogPlugin, prelude::*,
    state::app::StatesPlugin,
};
use clap::Parser;
use game::network::Sess;
use lightyear::prelude::{ConnectToken, generate_key};
use lobby_common::{LobbyToServer, ServerToLobby};
use tokio::sync::mpsc;
use wtransport::{Endpoint, Identity, ServerConfig};

#[derive(clap::Parser)]
struct Options {
    port: u16,
}

#[tokio::main]
async fn main() -> AppExit {
    let options = Options::parse();

    let addr = public_ip_address::perform_lookup(None).await.unwrap().ip;

    // Wait for connection from lobby server
    let server = Endpoint::server(
        ServerConfig::builder()
            .with_bind_default(options.port)
            .with_identity(
                Identity::self_signed(["localhost", "127.0.0.1", "::1", "moba.elekrisk.com"])
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
        return AppExit::Error(1.try_into().unwrap());
    };

    let private_key = generate_key();

    let mut tokens = HashMap::new();

    for player in players {
        let token = ConnectToken::build(
            (addr, options.port),
            0,
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

    App::new()
        .add_plugins((
            MinimalPlugins,
            LogPlugin::default(),
            TransformPlugin::default(),
            DiagnosticsPlugin::default(),
            TerminalCtrlCHandlerPlugin::default(),
            AssetPlugin::default(),
            StatesPlugin::default(),
            game::server,
        ))
        .run()
}

// /home/elekrisk/projects/moba_try_6/game
// /home/elekrisk/projects/moba_try_6/game
