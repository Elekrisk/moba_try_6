use bevy::prelude::*;

use std::{rc::Rc, sync::Arc, time::Duration};

use lobby_common::{ClientToLobby, LobbyToClient};
use serde::{Deserialize, Serialize};
use tokio::{
    io::AsyncReadExt as _,
    sync::{
        mpsc::{self, UnboundedReceiver, UnboundedSender},
        oneshot::Sender,
    },
};
use wtransport::{Connection, endpoint::endpoint_side::Client};
use xwt_core::base::Session;
use xwt_wtransport::wtransport::{
    ClientConfig, Endpoint,
    config::TlsClientConfig,
    error::ConnectingError,
    tls::rustls::{
        self, DigitallySignedStruct, SignatureScheme,
        client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
        pki_types::CertificateDer,
    },
};

use crate::r#async::AsyncContext;

use super::{LobbyMessage, Sess};

pub fn create_endpoint() -> Endpoint<Client> {
    let config = ClientConfig::builder()
        .with_bind_default()
        .with_no_cert_validation()
        .keep_alive_interval(Some(Duration::from_secs(1)))
        .max_idle_timeout(Some(Duration::from_secs(5)))
        .unwrap()
        .build();

    Endpoint::client(config).unwrap()
}

pub async fn connect(address: String) -> anyhow::Result<Sess<xwt_wtransport::Connection>> {
    Ok(Sess(Arc::new(xwt_wtransport::Connection(
        create_endpoint()
            .connect(format!("https://{address}"))
            .await?,
    ))))
}

// pub struct Connection<X: Session> {
//     x: X,
// }

pub trait SendMessage {
    async fn send<T: Serialize>(&self, msg: T) -> anyhow::Result<()>;
}

impl SendMessage for Connection {
    async fn send<T: Serialize>(&self, msg: T) -> anyhow::Result<()> {
        let msg = serde_json::to_vec_pretty(&msg)?;
        self.open_uni().await?.await?.write_all(&msg).await?;
        Ok(())
    }
}

pub trait RecvMessage {
    async fn recv<T: for<'de> Deserialize<'de>>(&self) -> anyhow::Result<T>;
}

impl RecvMessage for Connection {
    async fn recv<T: for<'de> Deserialize<'de>>(&self) -> anyhow::Result<T> {
        let mut buf = vec![];
        self.accept_uni().await?.read_to_end(&mut buf).await?;
        Ok(serde_json::from_slice(&buf)?)
    }
}
