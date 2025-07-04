use clap::Parser;
use coupe::{Config, Result};
use coupe_sentinel::{serve_api, watch_sessions};
use mimalloc::MiMalloc;
use std::{path::PathBuf, sync::Arc};
use tokio::spawn;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

async fn run(config: Config) -> Result<()> {
    let config = Arc::new(config);
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
    let cli = Cli::parse();
    let config = match Config::load(cli.config) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };
    if let Err(e) = run(config).await {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
