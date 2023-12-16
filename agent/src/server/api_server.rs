use std::{collections::BTreeSet, sync::Mutex};

use axum::{response::IntoResponse, routing::get, Router};
use ipnet::IpNet;
use once_cell::sync::{Lazy, OnceCell};
use tracing::warn;

static SUBNET: OnceCell<IpNet> = OnceCell::new();
static IP_STORE: Lazy<Mutex<BTreeSet<String>>> = Lazy::new(|| match SUBNET.get() {
    Some(subnet) => subnet
        .hosts()
        .skip(1)
        .map(|ip| ip.to_string())
        .collect::<BTreeSet<String>>()
        .into(),
    None => BTreeSet::new().into(),
});

#[tokio::main]
pub async fn start(pod_cidr: &str) -> anyhow::Result<()> {
    SUBNET
        .set(pod_cidr.parse::<IpNet>()?)
        .map_or_else(|_| warn!("setup subnet failed"), |_| {});

    let app = Router::new()
        .route("/", get(root))
        .route("/ipam/ip", get(pop_first));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn root() -> &'static str {
    "Hello, world!"
}

async fn pop_first() -> impl IntoResponse {
    IP_STORE.lock().unwrap().pop_first().unwrap_or_default()
}
