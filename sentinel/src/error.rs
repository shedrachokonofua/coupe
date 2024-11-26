use axum::{ body::Body, http::StatusCode, response::{ IntoResponse, Response }, Json };
use serde_json::json;
use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum SentinelError {
    #[error("Container {0} not found")] ContainerNotFound(String),
    #[error("Container {0} not recoverable")] ContainerNotRecoverable(String),
    #[error(
        "Container {0} failed to enter running state within timeout period"
    )] ContainerStartupTimeout(String),
    #[error(
        "Request to container daemon failed for unknown reason: {0}"
    )] ContainerDaemonRequestFailed(String),
    #[error("Internal server error: {0}")] InternalServerError(String),
}

impl IntoResponse for SentinelError {
    fn into_response(self) -> Response<Body> {
        (
            match self {
                SentinelError::ContainerNotFound(_) => { StatusCode::NOT_FOUND }
                SentinelError::ContainerNotRecoverable(_) => { StatusCode::GONE }
                SentinelError::ContainerDaemonRequestFailed(_) => {
                    StatusCode::INTERNAL_SERVER_ERROR
                }
                SentinelError::ContainerStartupTimeout(_) => { StatusCode::GATEWAY_TIMEOUT }
                SentinelError::InternalServerError(_) => { StatusCode::INTERNAL_SERVER_ERROR }
            },
            Json(json!({ "error": self.to_string() })),
        ).into_response()
    }
}

impl From<anyhow::Error> for SentinelError {
    fn from(e: anyhow::Error) -> Self {
        Self::InternalServerError(e.to_string())
    }
}
