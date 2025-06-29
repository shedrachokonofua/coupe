use clap::Parser;
use coupe_cli::{Cli, execute};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    if let Err(e) = execute(cli.command).await {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
