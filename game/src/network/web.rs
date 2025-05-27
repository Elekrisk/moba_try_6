use crate::network::Sess;
use bevy::prelude::info;
use std::sync::Arc;
use xwt_core::endpoint::Connect;
use xwt_core::endpoint::connect::Connecting;
use xwt_web::{Endpoint, web_wt_sys::WebTransportOptions};

pub fn create_endpoint() -> Endpoint {
    Endpoint {
        options: WebTransportOptions::new(),
    }
}

pub async fn connect(address: String) -> anyhow::Result<Sess<xwt_web::Session>> {
    let endpoint = create_endpoint();
    info!("Connecting step 1...");
    let session = endpoint
        .connect(&format!("https://{address}"))
        .await
        .handle_err()?;
    info!("Connecting step 2...");
    let session = session.wait_connect().await.handle_err()?;
    info!("Connected");

    Ok(Sess(Arc::new(session)))
}

trait HandleErr<T> {
    fn handle_err(self) -> anyhow::Result<T>;
}

impl<T, E: std::error::Error> HandleErr<T> for Result<T, E> {
    fn handle_err(self) -> anyhow::Result<T> {
        self.map_err(|e| anyhow::anyhow!("Error: {e}"))
    }
}
