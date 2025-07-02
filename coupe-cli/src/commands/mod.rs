pub mod deploy;
pub mod new;
pub mod teardown;

use crate::{Commands, Result};

pub async fn execute(command: Commands) -> Result<()> {
    match command {
        Commands::New { name, path } => new::execute(name, path).await,
        Commands::Deploy { path, remote } => deploy::execute(path, remote).await,
        Commands::Teardown { path, remote } => teardown::execute(path, remote).await,
    }
}
