use clap::Parser;
use coupe::Config;
use coupe_sentinel::{AppError, Result, serve_api};
use mimalloc::MiMalloc;
use std::path::PathBuf;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

async fn run(config: Config) -> Result<()> {
    serve_api(config).await
}

#[derive(Parser)]
#[command(name = "coupe-sentinel")]
#[command(about = "Coupe Sentinel API server", long_about = None)]
struct Cli {
    /// Path to the coupe stack configuration file
    #[arg(short, long, default_value = "coupe.yaml")]
    config: PathBuf,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let config = match Config::load(cli.config).map_err(|e| AppError::Config(e.to_string())) {
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
