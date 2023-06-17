use std::net::SocketAddr;

use axum::{extract::State, response::IntoResponse, routing::get, Json, Router};
use prometheus::{Encoder, TextEncoder};
use sinabro::{controller, telemetry};

#[tokio::main]
async fn main() {
    telemetry::init().await;

    // Initialize Kubernetes controller state
    let state = controller::State::default();
    let controller = controller::run(state.clone());

    let app = Router::new()
        .route("/", get(index))
        .route("/metrics", get(metrics))
        .route("/health", get(health))
        .with_state(state.clone());

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn index(State(state): State<controller::State>) -> impl IntoResponse {
    let d = state.diagnostics().await;
    Json(d)
}

async fn metrics(State(state): State<controller::State>) -> impl IntoResponse {
    let metrics = state.metrics();
    let encoder = TextEncoder::new();
    let mut buf = vec![];

    encoder.encode(&metrics, &mut buf).unwrap();
    buf
}

async fn health() -> impl IntoResponse {
    Json("helthy")
}
