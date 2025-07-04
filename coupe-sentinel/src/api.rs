use axum::{
    Json, Router, extract::State, http::StatusCode, response::IntoResponse, routing::get, serve,
};
use coupe::{Config, CoupeError, Result};
use serde_json::json;
use std::{collections::HashMap, sync::Arc};
use tokio::net::TcpListener;
use tracing::{error, info};

use crate::get_all_sessions;

fn get_route_mapping(config: &Config) -> HashMap<String, coupe::Function> {
    let mut mapping = HashMap::new();
    for function in config.functions.values() {
        if let coupe::Trigger::Http { path: route, .. } = &function.trigger {
            mapping.insert(route.clone(), function.clone());
        }
    }
    mapping
}

async fn health() -> impl IntoResponse {
    Json(json!({ "running": true }))
}

async fn list_sessions() -> impl IntoResponse {
    match get_all_sessions().await {
        Ok(sessions) => (StatusCode::OK, Json(sessions)).into_response(),
        Err(e) => {
            error!(error = e.to_string().as_str(), "Failed to list sessions");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

async fn get_config(State(config): State<Arc<Config>>) -> impl IntoResponse {
    match serde_json::to_value(&*config) {
        Ok(res) => (StatusCode::OK, Json(res)),
        Err(e) => {
            error!(error = e.to_string().as_str(), "Failed to get config");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            )
        }
    }
    .into_response()
}

pub async fn serve_api(config: Arc<Config>) -> Result<()> {
    let router = Router::new()
        .route("/health", get(health))
        .route("/system/sessions", get(list_sessions))
        .route("/system/config", get(get_config))
        .with_state(Arc::clone(&config));
    let port = config.sentinel_port();
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .map_err(|e| CoupeError::Io(e))?;
    info!("Listening on port {}", port);
    serve(listener, router)
        .await
        .map_err(|e| CoupeError::Io(e))?;
    Ok(())
}
