use axum::{
    extract::{Path, State},
    response::IntoResponse,
    routing::{get, put},
    Router,
};
use tokio::signal::unix::{signal, SignalKind};
use tracing::warn;

use super::{ipam::Ipam, state::AppState};

#[tokio::main]
pub async fn start(pod_cidr: &str) -> anyhow::Result<()> {
    let store_path = "/var/lib/sinabro/ip_store"; // TODO: make this configurable
    let ipam = Ipam::new(pod_cidr, store_path);
    let ipam_clone = ipam.clone();
    let state = AppState { ipam };

    tokio::spawn(handle_signals(ipam_clone));

    let app = Router::new()
        .route("/", get(root))
        .route("/ipam/ip", get(pop_first))
        .route("/ipam/ip/:ip", put(insert))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn root() -> &'static str {
    "Hello, world!"
}

async fn pop_first(State(ipam): State<Ipam>) -> impl IntoResponse {
    ipam.pop_first().unwrap_or_default()
}

async fn insert(State(ipam): State<Ipam>, Path(ip): Path<String>) {
    ipam.insert(&ip);
}

async fn handle_signals(ipam: Ipam) {
    let mut sigterm = signal(SignalKind::terminate()).unwrap();
    let mut sigint = signal(SignalKind::interrupt()).unwrap();

    tokio::select! {
        _ = sigterm.recv() => {},
        _ = sigint.recv() => {},
    };

    ipam.flush()
        .unwrap_or_else(|_| warn!("flush ip store failed"));
}
