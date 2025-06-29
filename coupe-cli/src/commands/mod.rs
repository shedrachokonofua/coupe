pub mod deploy;
pub mod new;

use crate::{Commands, Result};

pub async fn execute(command: Commands) -> Result<()> {
    match command {
        Commands::New { name, path } => new::execute(name, path).await,
        Commands::Deploy { path } => deploy::execute(path).await,
    }
}
