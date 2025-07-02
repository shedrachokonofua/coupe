use axum::{Json, Router, response::IntoResponse, routing::get, serve};
use coupe::Config;
use serde_json::json;
use tokio::net::TcpListener;

use crate::{AppError, Result};

async fn health() -> impl IntoResponse {
    Json(json!({ "running": true }))
}

pub async fn serve_api(config: Config) -> Result<()> {
    let router = Router::new().route("/health", get(health));
    let port = config.sentinel.unwrap_or_default().port.unwrap_or(52345);
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .map_err(|e| AppError::Io(e))?;
    serve(listener, router).await.map_err(|e| AppError::Io(e))?;
    Ok(())
}
