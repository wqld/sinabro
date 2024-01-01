use std::sync::Arc;

use axum::{
    extract::{Path, State},
    response::IntoResponse,
    routing::{get, put},
    Router,
};
use tokio::{
    signal::{self},
    sync::Notify,
};
use tracing::warn;

use super::{ipam::Ipam, state::AppState};

pub async fn start(pod_cidr: &str, store_path: &str, shutdown: Arc<Notify>) -> anyhow::Result<()> {
    let ipam = Ipam::new(pod_cidr, store_path);
    let ipam_clone = ipam.clone();

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app(ipam))
        .with_graceful_shutdown(shutdown_signal(shutdown))
        .await
        .unwrap();

    ipam_clone
        .flush()
        .unwrap_or_else(|_| warn!("flush ip store failed"));

    Ok(())
}

fn app(ipam: Ipam) -> Router {
    let state = AppState { ipam };
    Router::new()
        .route("/", get(root))
        .route("/ipam/ip", get(pop_first))
        .route("/ipam/ip/:ip", put(insert))
        .with_state(state)
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

async fn shutdown_signal(shutdown: Arc<Notify>) {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
        _ = shutdown.notified() => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Method, Request},
    };
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_start() {
        let pod_cidr = "10.244.0.0/24";
        let tmp_dir = tempfile::tempdir().unwrap();
        let store_path = Arc::new(tmp_dir.path().join("ip_store"));
        let store_path_clone = store_path.clone();
        let shutdown = Arc::new(Notify::new());
        let shutdown_clone = shutdown.clone();

        let server = tokio::spawn(async move {
            start(pod_cidr, store_path.to_str().unwrap(), shutdown_clone)
                .await
                .unwrap();
        });

        let notify = tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            shutdown.notify_one();
        });

        tokio::try_join!(server, notify).unwrap();

        assert!(std::path::Path::new(store_path_clone.to_str().unwrap()).exists());
        assert_eq!(
            std::fs::read_to_string(store_path_clone.to_str().unwrap())
                .unwrap()
                .lines()
                .count(),
            253
        );
    }

    #[tokio::test]
    async fn test_get_ipam_ip() {
        let pod_cidr = "10.244.0.0/24";
        let tmp_dir = tempfile::tempdir().unwrap();
        let store_path = tmp_dir.path().join("ip_store");
        let ipam = Ipam::new(pod_cidr, store_path.to_str().unwrap());
        let app = app(ipam);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/ipam/ip")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), 200);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], b"10.244.0.2");
    }

    #[tokio::test]
    async fn test_put_ipam_ip() {
        let pod_cidr = "10.244.0.0/24";
        let tmp_dir = tempfile::tempdir().unwrap();
        let store_path = tmp_dir.path().join("ip_store");
        let ipam = Ipam::new(pod_cidr, store_path.to_str().unwrap());
        let ipam_clone = ipam.clone();
        let app = app(ipam);

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::PUT)
                    .uri("/ipam/ip/10.244.0.1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), 200);

        let result = ipam_clone.pop_first().unwrap();
        assert_eq!(result, "10.244.0.1");
    }
}
