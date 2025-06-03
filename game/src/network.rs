use std::sync::Arc;

use anyhow::bail;
use bevy::prelude::*;
use lobby_common::{
    ClientToLobby, LobbyToClient, PlayerId,
};
use serde::{Deserialize, Serialize};
use tokio::{
    select,
    sync::
        mpsc::{self, UnboundedReceiver, UnboundedSender}
    ,
};
use xwt_core::{
    base::Session,
    prelude::AsErrorCodeExt,
    session::stream::OpeningUni,
    stream::{Read, Write},
};

use crate::r#async::AsyncContext;

#[cfg(not(target_family = "wasm"))]
#[path = "network/native.rs"]
mod inner;
#[cfg(target_family = "wasm")]
#[path = "network/web.rs"]
mod inner;

pub fn client(_app: &mut App) {}

pub struct ConnectToLobbyCommand(pub String);

impl Command for ConnectToLobbyCommand {
    fn apply(self, world: &mut World) {
        world
            .run_system_cached_with(connect_to_lobby_server, self.0)
            .unwrap();
    }
}

#[derive(Debug, Event)]
pub struct LobbyConnectionFailed(pub anyhow::Error);

#[derive(Resource, Deref)]
pub struct LobbySender(pub UnboundedSender<ClientToLobby>);
#[derive(Resource)]
pub struct LobbyReceiver(pub UnboundedReceiver<LobbyMessage>);

fn connect_to_lobby_server(
    address: In<String>,
    runtime: Res<AsyncContext>,
    mut commands: Commands,
) {
    let (send_to_lobby, recv_from_client) = mpsc::unbounded_channel();
    let (send_internal, recv_internal) = mpsc::unbounded_channel();

    commands.insert_resource(LobbySender(send_to_lobby));
    commands.insert_resource(LobbyReceiver(recv_internal));

    runtime.run(async move {
        match connect(address.0, recv_from_client, send_internal).await {
            Ok(()) => {}
            Err(e) => warn!("Lobby connection error: {e}"),
        }
    });
}

async fn connect(
    address: String,
    mut recv_from_client: UnboundedReceiver<ClientToLobby>,
    send_internal: UnboundedSender<LobbyMessage>,
) -> anyhow::Result<()> {
    match inner::connect(address).await {
        Ok(connection) => {
            connection
                .send(ClientToLobby::Handshake {
                    name: whoami::username(),
                })
                .await
                .unwrap();
            let LobbyToClient::Handshake { id } = connection.recv::<LobbyToClient>().await.unwrap()
            else {
                bail!("Invalid handshake");
            };
            send_internal.send(LobbyMessage::LobbyConnected(id))?;

            loop {
                let read_message = connection.recv::<LobbyToClient>();

                select! {
                    msg = read_message => {
                        match msg {
                            Ok(msg) => send_internal.send(LobbyMessage::Message(msg))?,
                            Err(err) => {
                                warn!("Connection lost: {err}");
                                send_internal.send(LobbyMessage::ConnectionLost)?;
                                break
                            },
                        }
                    },
                    msg = recv_from_client.recv() => {
                        match msg {
                            Some(msg) => connection.send(msg).await?,
                            None => {
                                warn!("Channel closed");
                                break
                            }
                        }
                    }
                }
            }
        }
        Err(err) => {
            warn!("Connection error: {err:?}");
            send_internal.send(LobbyMessage::LobbyConnectionFailed(err))?;
        }
    }

    Ok(())
}

#[derive(Debug)]
pub enum LobbyMessage {
    LobbyConnected(PlayerId),
    LobbyConnectionFailed(anyhow::Error),
    ConnectionLost,
    Message(LobbyToClient),
}

// async fn send_to_server(
//     conn: Connection,
//     mut r: UnboundedReceiver<ClientToLobby>,
// ) -> anyhow::Result<()> {
//     loop {
//         use anyhow::anyhow;
//         let msg = r.recv().await.ok_or(anyhow!("Failed to read"))?;
//         conn.send(msg).await?;
//     }
// }

// async fn listen_to_lobby_server(
//     connection: Connection,
//     s: UnboundedSender<LobbyMessage>,
// ) -> anyhow::Result<()> {
//     s.send(LobbyMessage::LobbyConnected)?;
//     loop {
//         let Ok(msg) = connection.recv::<LobbyToClient>().await else {
//             s.send(LobbyMessage::ConnectionLost)?;
//             break;
//         };

//         s.send(LobbyMessage::Message(msg))?;
//     }

//     Ok(())
// }

#[derive(Clone, Deref, DerefMut)]
pub struct Sess<S: Session>(pub Arc<S>);

impl<S: Session> Sess<S> {
    pub async fn send<T: Serialize>(&self, msg: T) -> anyhow::Result<()> {
        let msg = serde_json::to_vec_pretty(&msg)?;
        let mut stream = self
            .open_uni()
            .await
            .map_err(|err| anyhow::anyhow!("Failed opening uni: {err}"))?
            .wait_uni()
            .await
            .map_err(|err| anyhow::anyhow!("Failed opening uni: {err}"))?;
        let mut from = 0;
        while from < msg.len() {
            from += usize::from(
                stream
                    .write(&msg[from..])
                    .await
                    .map_err(|err| anyhow::anyhow!("Failed writing message: {err}"))?,
            );
        }
        Ok(())
    }

    pub async fn recv<T: for<'de> Deserialize<'de>>(&self) -> anyhow::Result<T> {
        let mut stream = self
            .accept_uni()
            .await
            .map_err(|err| anyhow::anyhow!("Failed accepting uni: {err}"))?;
        let mut buf = vec![];
        let mut temp_buf = vec![0; 1024];
        loop {
            match stream.read(&mut temp_buf).await {
                Ok(num) => buf.extend_from_slice(&temp_buf[..usize::from(num)]),
                Err(err) if err.is_closed() => break,
                Err(err) => bail!("Failed reading message: {err}"),
            }
        }
        Ok(serde_json::from_slice(&buf)?)
    }
}
