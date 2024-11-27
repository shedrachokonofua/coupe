use std::time::Duration;
use axum::{ http::StatusCode, response::IntoResponse, routing::{ get, post }, serve, Json, Router };
use anyhow::Result;
use crate::{ error::SentinelError, sessions::end_session };
use serde::Deserialize;
use serde_json::json;
use crate::sessions::{ get_all_sessions, start_session };
use tokio::net::TcpListener;
use axum_tracing_opentelemetry::middleware::{ OtelAxumLayer, OtelInResponseLayer };
use tracing::instrument;
use tower::ServiceBuilder;

#[instrument]
async fn health() -> impl IntoResponse {
    Json(json!({ "running": true }))
}

#[derive(Deserialize, Debug)]
struct PostSessionRequest {
    function_name: String,
    duration_seconds: Option<u64>,
}

#[instrument]
async fn handle_start_session(Json(payload): Json<PostSessionRequest>) -> impl IntoResponse {
    let duration = payload.duration_seconds
        .map(Duration::from_secs)
        .unwrap_or_else(|| Duration::from_secs(60 * 15));

    match start_session(payload.function_name.clone(), duration, Default::default()).await {
        Ok(session) =>
            (
                StatusCode::OK,
                Json(
                    json!({
                        "function_name": session.function_name,
                        "ends_at": session.ends_at,
                    })
                ),
            ).into_response(),

        Err(e) => {
            if let Some(sentinel_error) = e.downcast_ref::<SentinelError>() {
                sentinel_error.clone().into_response()
            } else {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": e.to_string() })),
                ).into_response()
            }
        }
    }
}

#[instrument]
async fn handle_get_sessions() -> impl IntoResponse {
    match get_all_sessions().await {
        Ok(sessions) =>
            (
                StatusCode::OK,
                Json(json!({
                    "sessions": sessions,
                })),
            ).into_response(),

        Err(e) => {
            if let Some(sentinel_error) = e.downcast_ref::<SentinelError>() {
                sentinel_error.clone().into_response()
            } else {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": e.to_string() })),
                ).into_response()
            }
        }
    }
}

#[derive(Deserialize, Debug)]
struct DeleteSessionRequest {
    function_name: String,
}

#[instrument]
async fn handle_delete_session(Json(payload): Json<DeleteSessionRequest>) -> impl IntoResponse {
    match end_session(payload.function_name.clone()).await {
        Ok(_) =>
            (
                StatusCode::OK,
                Json(json!({
                    "message": "Session deleted",
                })),
            ).into_response(),
        Err(e) => {
            if let Some(sentinel_error) = e.downcast_ref::<SentinelError>() {
                sentinel_error.clone().into_response()
            } else {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": e.to_string() })).into_response(),
                ).into_response()
            }
        }
    }
}

fn router() -> Router {
    Router::new()
        .route("/health", get(health))
        .route(
            "/sessions",
            post(handle_start_session).get(handle_get_sessions).delete(handle_delete_session)
        )
}

pub async fn start_api() -> Result<()> {
    let app = router()
        .layer(
            ServiceBuilder::new()
                .layer(OtelAxumLayer::default())
                .layer(OtelInResponseLayer::default())
        )
        .into_make_service();
    let listener = TcpListener::bind("0.0.0.0:8081").await?;
    serve(listener, app).await?;
    Ok(())
}
