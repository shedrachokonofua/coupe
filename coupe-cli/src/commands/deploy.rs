use crate::{AppError, Result};
use coupe::{Config, DeploymentTarget, StackDockerClient};
use std::path::PathBuf;

pub async fn execute(path: Option<String>, remote: Option<String>) -> Result<()> {
    println!("Deploying coupe stack");
    let deployment_target = if let Some(remote) = remote {
        DeploymentTarget::Remote(remote)
    } else {
        DeploymentTarget::Local
    };
    let config_path = path.unwrap_or("coupe.yaml".to_string());
    let config =
        Config::load(PathBuf::from(config_path)).map_err(|e| AppError::Config(e.to_string()))?;
    let stack_docker_client = StackDockerClient::new(&config, &deployment_target)
        .map_err(|e| AppError::Docker(e.to_string()))?;

    stack_docker_client
        .teardown()
        .await
        .map_err(|e| AppError::Docker(e.to_string()))?;

    stack_docker_client
        .create_network()
        .await
        .map_err(|e| AppError::Docker(e.to_string()))?;

    stack_docker_client
        .create_containers()
        .await
        .map_err(|e| AppError::Docker(e.to_string()))?;

    stack_docker_client
        .ensure_sentinel_running()
        .await
        .map_err(|e| AppError::Docker(e.to_string()))?;

    Ok(())
}
