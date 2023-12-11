use axum::{routing::get, Router};

#[tokio::main]
pub async fn start() -> anyhow::Result<()> {
    let app = Router::new().route("/", get(root));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn root() -> &'static str {
    "Hello, world!"
}
