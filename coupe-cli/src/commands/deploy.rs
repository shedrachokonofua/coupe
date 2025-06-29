use crate::{Result, AppError};
use coupe_config::Config;
use std::fs;
use std::path::{PathBuf};

fn load_config(path: Option<String>) -> Result<Config> {
    let config_path = match path {
        Some(p) => PathBuf::from(p),
        None => {
            PathBuf::from("coupe.yaml")
        }
    };

    if !config_path.exists() {
        return Err(AppError::InvalidInput(format!(
            "Config file not found: {}",
            config_path.display()
        )));
    }

    let config_content = fs::read_to_string(&config_path)?;
    let config: Config = serde_yaml::from_str(&config_content)?;
    Ok(config)
}

pub async fn execute(path: Option<String>) -> Result<()> {
    println!("Deploying coupe stack");

    let config = load_config(path)?;

    println!("Config: {:?}", config);

    Ok(())
}
