use crate::{Config, CoupeError, Result, recreate_docker_stack};
use remotefs::{
    RemoteErrorType, RemoteFs,
    fs::{Metadata, UnixPex},
};
use remotefs_ssh::{SftpFs, SshOpts};
use std::{
    env,
    path::{Path, PathBuf},
};
use tokio::fs;

pub enum DeploymentTarget {
    Local,
    Remote(String),
}

fn connect_ssh(host: &str) -> Result<SftpFs> {
    let mut client: SftpFs = SshOpts::new(host).into();
    client
        .connect()
        .map_err(|e| CoupeError::SshConnection(e.to_string()))?;
    Ok(client)
}

pub fn deployment_path(config: &Config) -> PathBuf {
    let home = env::home_dir().unwrap_or("/home".into());
    Path::new(&home).join(".coupe").join(&config.name)
}

fn config_path(config: &Config) -> PathBuf {
    deployment_path(config).join("coupe.yaml")
}

pub async fn deploy_config(config: &Config, target: &DeploymentTarget) -> Result<()> {
    if let DeploymentTarget::Remote(host) = target {
        let mut client = connect_ssh(host)?;
        client
            .create_dir(deployment_path(config).as_path(), UnixPex::from(0o755))
            .or_else(|e| {
                if let RemoteErrorType::DirectoryAlreadyExists = e.kind {
                    Ok(())
                } else {
                    Err(CoupeError::SshCommand(e.to_string()))
                }
            })?;

        let yaml_content = serde_yaml::to_string(config).map_err(|e| CoupeError::Yaml(e))?;
        let reader = std::io::Cursor::new(yaml_content);
        client
            .create_file(
                config_path(config).as_path(),
                &Metadata::default(),
                Box::new(reader),
            )
            .map_err(|e| CoupeError::SshCommand(e.to_string()))?;
        client
            .disconnect()
            .map_err(|e| CoupeError::SshCommand(e.to_string()))?;
    } else {
        println!("Deploying to local filesystem");
        fs::create_dir_all(deployment_path(config)).await?;
        fs::write(config_path(config), serde_yaml::to_string(config)?).await?;
    }
    Ok(())
}

pub async fn remove_config(config: &Config, target: &DeploymentTarget) -> Result<()> {
    if let DeploymentTarget::Remote(host) = target {
        let mut client = connect_ssh(host)?;
        if client
            .exists(deployment_path(config).as_path())
            .inspect_err(|e| println!("Error checking if directory exists: {}", e))
            .unwrap_or(false)
        {
            client
                .remove_dir(deployment_path(config).as_path())
                .map_err(|e| CoupeError::SshCommand(e.to_string()))?;
            client
                .disconnect()
                .map_err(|e| CoupeError::SshCommand(e.to_string()))?;
        }
    } else {
        if fs::metadata(deployment_path(config)).await.is_ok() {
            fs::remove_dir_all(deployment_path(config)).await?;
        }
    }
    Ok(())
}

pub async fn deploy_stack(config: &Config, target: &DeploymentTarget) -> Result<()> {
    deploy_config(config, target).await?;
    recreate_docker_stack(config, target).await?;
    Ok(())
}
