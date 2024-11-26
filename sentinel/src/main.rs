use api::start_api;
use config::CONFIG;
use coupe_lib::telemetry::{ Telemetry, TelemetryConfig };
use anyhow::Result;
use nats::{ has_nats_triggers, watch_nats_triggers };
use sessions::watch_sessions;
use tokio::{ main, spawn };
use mimalloc::MiMalloc;

mod api;
mod config;
mod containers;
mod db;
mod error;
mod nats;
mod sessions;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[main]
async fn main() -> Result<()> {
    Telemetry::init(TelemetryConfig {
        otel_endpoint: CONFIG.otel_endpoint.clone(),
        service_name: "sentinel".to_string(),
        container_name: "sentinel".to_string(),
    })?;
    spawn(async {
        watch_sessions().await.expect("Failed to watch sessions");
    });
    if has_nats_triggers() {
        spawn(async {
            watch_nats_triggers().await.expect("Failed to run NATS consumer waker");
        });
    }
    start_api().await?;
    Ok(())
}
