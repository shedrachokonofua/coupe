use crate::{DOCKER_CLIENT, get_all_sessions, start_session};
use axum::{
    Json, Router,
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    response::IntoResponse,
    routing::{any, delete, get, patch, post, put},
    serve,
};
use axum_proxy::Identity;
use coupe::{Config, CoupeError, HttpMethod, Result, ensure_function_running};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_service::Service;
use tracing::{error, info};

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
fn build_function_router(config: Arc<Config>) -> Result<Router> {
    let mut router = Router::new();
    for function_name in config.http_functions() {
        let function = config
            .functions
            .get(&function_name)
            .ok_or(CoupeError::InvalidInput(format!(
                "Function not found: {}",
                function_name
            )))?;
        let (path, method, _schema, _auth) =
            function
                .trigger
                .clone()
                .as_http()
                .ok_or(CoupeError::InvalidInput(format!(
                    "Function {} is not an HTTP function",
                    function_name
                )))?;
        let reverse_proxy = axum_proxy::builder_http(
            config.internal_function_url(&function_name).map_err(|e| {
                CoupeError::InvalidInput(format!("Failed to get function URL: {}", e.to_string()))
            })?,
        )
        .map_err(|e| CoupeError::InvalidInput(format!("Failed to build reverse proxy: {}", e)))?
        .build(Identity);

        let handler_config = Arc::clone(&config);
        let handler_function_name = function_name.clone();

        let handler = move |request: Request<Body>| {
            let mut proxy = reverse_proxy.clone();
            let config = handler_config.clone();
            let function_name = handler_function_name.clone();

            async move {
                if let Err(e) = start_session(&config, function_name.clone()).await {
                    error!(error = e.to_string().as_str(), "Failed to start session");
                    let res = Json(json!({ "error": e.to_string() }));
                    return match e {
                        CoupeError::Healthcheck(e) => {
                            (StatusCode::SERVICE_UNAVAILABLE, Json(json!({ "error": e })))
                        }
                        _ => (StatusCode::INTERNAL_SERVER_ERROR, res),
                    }
                    .into_response();
                }
                match proxy.call(request).await {
                    Ok(Ok(res)) => res.into_response(),
                    Ok(Err(e)) => {
                        error!(error = e.to_string().as_str(), "Proxy error");
                        (
                            StatusCode::BAD_GATEWAY,
                            Json(json!({ "error": "Proxy error", "message": e.to_string() })),
                        )
                            .into_response()
                    }
                    Err(e) => {
                        error!(error = e.to_string().as_str(), "Proxy error");
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!({ "error": "Proxy error, should be unreachable", "message": e.to_string() })),
                        )
                            .into_response()
                    }
                }
            }
        };
        let method_router = match method.unwrap_or(HttpMethod::Any) {
            HttpMethod::Any => any(handler),
            HttpMethod::Get => get(handler),
            HttpMethod::Post => post(handler),
            HttpMethod::Put => put(handler),
            HttpMethod::Delete => delete(handler),
            HttpMethod::Patch => patch(handler),
        };
        router = match path.as_str() {
            "*" => router.fallback(method_router),
            _ => router.route(&path, method_router),
        };
    }
    Ok(router)
}

#[derive(Deserialize)]
struct StartFunctionRequest {
    function_name: String,
}

async fn start_function(
    State(config): State<Arc<Config>>,
    Json(request): Json<StartFunctionRequest>,
) -> impl IntoResponse {
    let function_name = request.function_name;

    match ensure_function_running(&DOCKER_CLIENT, &config, &function_name).await {
        Ok(_) => (
            StatusCode::OK,
            Json(json!({ "message": "Function started" })),
        ),
        Err(CoupeError::InvalidInput(e)) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))),
        Err(e) => {
            error!(error = e.to_string().as_str(), "Failed to start function");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            )
        }
    }
}

pub async fn serve_api(config: Arc<Config>) -> Result<()> {
    let mut router = Router::new()
        .route("/health", get(health))
        .route("/system/sessions", get(list_sessions))
        .route("/system/config", get(get_config))
        .route("/system/functions/start", post(start_function))
        .with_state(Arc::clone(&config));
    let function_router = build_function_router(Arc::clone(&config))?;
    router = router.fallback_service(function_router);
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
