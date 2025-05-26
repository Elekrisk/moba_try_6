use crate::network::Sess;
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
    let session = endpoint
        .connect(&format!("https://{address}"))
        .await
        .handle_err()?
        .wait_connect()
        .await
        .handle_err()?;

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
