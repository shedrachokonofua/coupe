use coupe::{Config, CoupeError, DeploymentTarget, Result, deploy_stack};
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
        Config::load(PathBuf::from(config_path)).map_err(|e| CoupeError::Config(e.to_string()))?;

    deploy_stack(&config, &deployment_target)
        .await
        .map_err(|e| CoupeError::Config(e.to_string()))?;

    Ok(())
}
