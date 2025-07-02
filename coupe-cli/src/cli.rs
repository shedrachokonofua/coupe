use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "coupe-cli")]
#[command(about = "Coupe CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    #[command(about = "Create a new coupe stack")]
    New {
        name: String,
        #[arg(short, long, help = "Path where the project should be created")]
        path: Option<String>,
    },
    #[command(about = "Deploy coupe stack")]
    Deploy {
        #[arg(short, long, help = "Path to the coupe.yaml file")]
        path: Option<String>,
        #[arg(short, long, help = "Remote host to deploy to")]
        remote: Option<String>,
    },
    #[command(about = "Remove a deployed coupe stack")]
    Teardown {
        #[arg(short, long, help = "Path to the coupe.yaml file")]
        path: Option<String>,
        #[arg(short, long, help = "Remote host to teardown")]
        remote: Option<String>,
    },
}
