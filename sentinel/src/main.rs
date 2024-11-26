use api::start_api;
use coupe_lib::telemetry::Telemetry;
use anyhow::Result;
use nats::watch_nats_triggers;
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
    Telemetry::init()?;
    spawn(async {
        watch_sessions().await.expect("Failed to watch sessions");
    });
    spawn(async {
        watch_nats_triggers().await.expect("Failed to run NATS consumer waker");
    });
    start_api().await?;
    Ok(())
}
