use crate::{Config, CoupeError, DeploymentTarget, Result};
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

pub struct ConfigTarget<'a> {
    config: &'a Config,
    target: &'a DeploymentTarget,
}

fn connect_ssh(host: &str) -> Result<SftpFs> {
    let mut client: SftpFs = SshOpts::new(host).into();
    client
        .connect()
        .map_err(|e| CoupeError::SshConnection(e.to_string()))?;
    Ok(client)
}

impl<'a> ConfigTarget<'a> {
    pub fn new(config: &'a Config, target: &'a DeploymentTarget) -> Self {
        Self { config, target }
    }

    pub fn dir_path(&self) -> PathBuf {
        let home = env::home_dir().unwrap_or("/home".into());
        Path::new(&home).join(".coupe").join(&self.config.name)
    }

    pub fn path(&self) -> PathBuf {
        self.dir_path().join("coupe.yaml")
    }

    pub async fn deploy(&self) -> Result<()> {
        if let DeploymentTarget::Remote(host) = self.target {
            let mut client = connect_ssh(host)?;
            client
                .create_dir(self.dir_path().as_path(), UnixPex::from(0o755))
                .or_else(|e| {
                    if let RemoteErrorType::DirectoryAlreadyExists = e.kind {
                        Ok(())
                    } else {
                        Err(CoupeError::SshCommand(e.to_string()))
                    }
                })?;

            let yaml_content =
                serde_yaml::to_string(self.config).map_err(|e| CoupeError::Yaml(e))?;
            let reader = std::io::Cursor::new(yaml_content);
            client
                .create_file(
                    self.path().as_path(),
                    &Metadata::default(),
                    Box::new(reader),
                )
                .map_err(|e| CoupeError::SshCommand(e.to_string()))?;
            client
                .disconnect()
                .map_err(|e| CoupeError::SshCommand(e.to_string()))?;
        } else {
            println!("Deploying to local filesystem");
            fs::create_dir_all(self.dir_path()).await?;
            fs::write(self.path(), serde_yaml::to_string(self.config)?).await?;
        }
        Ok(())
    }

    pub async fn remove(&self) -> Result<()> {
        if let DeploymentTarget::Remote(host) = self.target {
            let mut client = connect_ssh(host)?;
            if client
                .exists(self.dir_path().as_path())
                .inspect_err(|e| println!("Error checking if directory exists: {}", e))
                .unwrap_or(false)
            {
                client
                    .remove_dir(self.dir_path().as_path())
                    .map_err(|e| CoupeError::SshCommand(e.to_string()))?;
                client
                    .disconnect()
                    .map_err(|e| CoupeError::SshCommand(e.to_string()))?;
            }
        } else {
            if fs::metadata(self.dir_path()).await.is_ok() {
                fs::remove_dir_all(self.dir_path()).await?;
            }
        }
        Ok(())
    }
}
