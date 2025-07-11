use clap::Parser;
use coupe::{Config, Result};
use coupe_sentinel::{serve_api, watch_sessions};
use mimalloc::MiMalloc;
use std::{path::PathBuf, sync::Arc};
use tokio::spawn;
use tracing::{error, info};
use tracing_subscriber;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

async fn run(config: Config) -> Result<()> {
    let config = Arc::new(config);

    info!(
        stack_name = %config.name,
        sentinel_port = config.sentinel_port(),
        "Starting coupe-sentinel services"
    );

    spawn(watch_sessions(Arc::clone(&config)));
    serve_api(config).await
}

#[derive(Parser)]
#[command(name = "coupe-sentinel")]
#[command(about = "Coupe Sentinel API server", long_about = None)]
struct Cli {
    #[arg(short, long, default_value = "coupe.yaml")]
    config: PathBuf,
}

#[tokio::main]
async fn main() {
    // Initialize JSON logging with environment filter
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("coupe_sentinel=info,coupe=info"));

    tracing_subscriber::fmt()
        .json()
        .with_env_filter(env_filter)
        .with_target(true)
        .with_current_span(false)
        .with_span_list(true)
        .init();

    info!("Coupe Sentinel starting up");

    let cli = Cli::parse();
    info!(config_path = %cli.config.display(), "Loading configuration");

    let config = match Config::load(cli.config) {
        Ok(config) => {
            info!(stack_name = %config.name, "Configuration loaded successfully");
            config
        }
        Err(e) => {
            error!(error = %e, "Failed to load configuration");
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    if let Err(e) = run(config).await {
        error!(error = %e, "Runtime error occurred");
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
