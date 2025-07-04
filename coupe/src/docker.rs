use crate::{Config, CoupeError, DeploymentTarget, Result, deployment_path};
use bollard::API_DEFAULT_VERSION;
pub use bollard::Docker;
use bollard::errors::Error as BollardError;
use bollard::models::{ContainerCreateBody, ContainerStateStatusEnum, NetworkCreateRequest};
use bollard::query_parameters::{
    CreateContainerOptionsBuilder, InspectContainerOptions, RemoveContainerOptionsBuilder,
    StartContainerOptions, StopContainerOptions,
};
use bollard::secret::PortBinding;
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::{Instant, sleep};

const DEFAULT_SENTINEL_IMAGE: &str = "coupe/sentinel:latest";

pub fn connect_docker(target: &DeploymentTarget) -> Result<Docker> {
    match target {
        DeploymentTarget::Local => Docker::connect_with_unix_defaults(),
        DeploymentTarget::Remote(host) => Docker::connect_with_ssh(host, 30, API_DEFAULT_VERSION),
    }
    .map_err(|e| CoupeError::Docker(e.to_string()))
}

pub async fn create_sentinel_container(client: &Docker, config: &Config) -> Result<()> {
    let container_name = config.sentinel_container_name();
    let network_name = config.stack_network_name();

    let sentinel_image = config
        .sentinel
        .as_ref()
        .and_then(|s| s.registry.as_ref())
        .map(|r| {
            format!(
                "{}/{}/coupe-sentinel:latest",
                r.url,
                r.namespace.as_deref().unwrap_or("library")
            )
        })
        .unwrap_or_else(|| DEFAULT_SENTINEL_IMAGE.to_string());

    let bind_mount = format!("{}:/usr/app:rw", deployment_path(config).display());

    let container_config = ContainerCreateBody {
        image: Some(sentinel_image),
        env: Some(vec![format!("COUPE_STACK={}", config.name)]),
        labels: Some({
            let mut labels = HashMap::new();
            labels.insert("coupe.stack".to_string(), config.name.clone());
            labels.insert("coupe.role".to_string(), "sentinel".to_string());
            labels
        }),
        exposed_ports: Some(HashMap::from([(
            format!("{}/tcp", config.sentinel_port()),
            HashMap::<(), ()>::new(),
        )])),
        host_config: Some(bollard::models::HostConfig {
            network_mode: Some(network_name),
            binds: Some(vec![bind_mount]),
            port_bindings: Some(HashMap::from([(
                format!("{}/tcp", config.sentinel_port()),
                Some(vec![PortBinding {
                    host_ip: Some("0.0.0.0".to_string()),
                    host_port: Some(config.sentinel_port().to_string()),
                }]),
            )])),
            ..Default::default()
        }),
        ..Default::default()
    };

    let options = CreateContainerOptionsBuilder::new()
        .name(&container_name)
        .build();

    client
        .create_container(Some(options), container_config)
        .await
        .map_err(|e| CoupeError::Docker(e.to_string()))?;

    Ok(())
}

pub async fn create_function_container(
    client: &Docker,
    config: &Config,
    function_name: &str,
) -> Result<()> {
    let function_config = config
        .functions
        .get(function_name)
        .ok_or_else(|| CoupeError::Config(format!("Function {} not found", function_name)))?;

    let container_name = config.function_container_name(function_name);
    let network_name = config.stack_network_name();

    let container_config = ContainerCreateBody {
        image: Some(function_config.image.clone()),
        env: Some(vec![
            format!("COUPE_STACK={}", config.name),
            format!("COUPE_FUNCTION={}", function_name),
        ]),
        labels: Some({
            let mut labels = HashMap::new();
            labels.insert("coupe.stack".to_string(), config.name.clone());
            labels.insert("coupe.role".to_string(), "function".to_string());
            labels.insert("coupe.function".to_string(), function_name.to_string());
            labels
        }),
        host_config: Some(bollard::models::HostConfig {
            network_mode: Some(network_name),
            ..Default::default()
        }),
        ..Default::default()
    };

    let options = CreateContainerOptionsBuilder::new()
        .name(&container_name)
        .build();

    client
        .create_container(Some(options), container_config)
        .await
        .map_err(|e| CoupeError::Docker(e.to_string()))?;

    Ok(())
}

pub async fn create_containers(client: &Docker, config: &Config) -> Result<()> {
    create_sentinel_container(client, config).await?;
    for name in config.functions.keys() {
        create_function_container(client, config, name).await?;
    }
    Ok(())
}

pub async fn create_network(client: &Docker, config: &Config) -> Result<()> {
    let network_name = config.stack_network_name();

    let options = NetworkCreateRequest {
        name: network_name.clone(),
        driver: Some("bridge".to_string()),
        labels: Some({
            let mut labels = HashMap::new();
            labels.insert("coupe.stack".to_string(), config.name.clone());
            labels
        }),
        ..Default::default()
    };

    client
        .create_network(options)
        .await
        .map_err(|e| CoupeError::Docker(e.to_string()))?;

    Ok(())
}

pub async fn get_container_status(
    client: &Docker,
    container_id: &str,
) -> Result<ContainerStateStatusEnum> {
    let inspect_result = client
        .inspect_container(container_id, None::<InspectContainerOptions>)
        .await
        .map_err(|e| CoupeError::Docker(e.to_string()))?;

    let status = inspect_result
        .state
        .and_then(|state| state.status)
        .unwrap_or(ContainerStateStatusEnum::EMPTY);

    Ok(status)
}

pub struct ContainerRunResult {
    pub coldstarted: bool,
}

pub async fn ensure_container_running(
    client: &Docker,
    container_id: &str,
) -> Result<ContainerRunResult> {
    let status = get_container_status(client, container_id).await?;

    let coldstarted = match status {
        ContainerStateStatusEnum::RUNNING => true,
        ContainerStateStatusEnum::CREATED | ContainerStateStatusEnum::EXITED => {
            client
                .start_container(container_id, None::<StartContainerOptions>)
                .await
                .map_err(|e| CoupeError::Docker(e.to_string()))?;

            poll_until_running(client, container_id).await?;
            false
        }
        ContainerStateStatusEnum::PAUSED => {
            client
                .unpause_container(container_id)
                .await
                .map_err(|e| CoupeError::Docker(e.to_string()))?;

            poll_until_running(client, container_id).await?;
            false
        }
        ContainerStateStatusEnum::RESTARTING => {
            poll_until_running(client, container_id).await?;
            false
        }
        _ => {
            return Err(CoupeError::Docker(format!(
                "Container {} is in unrecoverable state: {:?}",
                container_id, status
            )));
        }
    };

    Ok(ContainerRunResult { coldstarted })
}

pub async fn ensure_sentinel_running(
    client: &Docker,
    config: &Config,
) -> Result<ContainerRunResult> {
    let container_name = config.sentinel_container_name();
    ensure_container_running(client, &container_name).await
}

pub async fn ensure_function_running(
    client: &Docker,
    config: &Config,
    function_name: &str,
) -> Result<ContainerRunResult> {
    let container_name = config.function_container_name(function_name);
    ensure_container_running(client, &container_name).await
}

pub async fn recreate_docker_stack(config: &Config, target: &DeploymentTarget) -> Result<()> {
    let client = connect_docker(target)?;
    teardown(&client, config).await?;
    create_network(&client, config).await?;
    create_containers(&client, config).await?;
    ensure_sentinel_running(&client, config).await?;
    Ok(())
}

async fn poll_until_running(client: &Docker, container_id: &str) -> Result<()> {
    let timeout = Duration::from_secs(30);
    let interval = Duration::from_millis(500);
    let start_time = Instant::now();

    while start_time.elapsed() < timeout {
        let status = get_container_status(client, container_id).await?;

        if status == ContainerStateStatusEnum::RUNNING {
            return Ok(());
        }

        sleep(interval).await;
    }

    Err(CoupeError::Docker(format!(
        "Container {} failed to start within timeout",
        container_id
    )))
}

pub async fn teardown(client: &Docker, config: &Config) -> Result<()> {
    println!("Tearing down stack");
    for name in config.functions.keys() {
        let container_name = config.function_container_name(name);
        println!("Removing container {}", container_name);
        remove_container_if_exists(client, &container_name).await?;
    }

    let sentinel_container_name = config.sentinel_container_name();
    remove_container_if_exists(client, &sentinel_container_name).await?;

    let network_name = config.stack_network_name();
    remove_network_if_exists(client, &network_name).await?;

    Ok(())
}

async fn remove_container_if_exists(client: &Docker, container_name: &str) -> Result<()> {
    let options = RemoveContainerOptionsBuilder::new().force(true).build();

    match client.remove_container(container_name, Some(options)).await {
        Ok(_) => Ok(()),
        Err(e) => {
            if let BollardError::DockerResponseServerError { status_code, .. } = &e {
                if *status_code == 404 {
                    return Ok(());
                }
            }
            Err(CoupeError::Docker(e.to_string()))
        }
    }
}

async fn remove_network_if_exists(client: &Docker, network_name: &str) -> Result<()> {
    match client.remove_network(network_name).await {
        Ok(_) => Ok(()),
        Err(e) => {
            if let BollardError::DockerResponseServerError { status_code, .. } = &e {
                if *status_code == 404 {
                    return Ok(());
                }
            }
            Err(CoupeError::Docker(e.to_string()))
        }
    }
}

async fn stop_container(client: &Docker, container_name: &str) -> Result<()> {
    client
        .stop_container(container_name, None::<StopContainerOptions>)
        .await
        .map_err(|e| CoupeError::Docker(e.to_string()))?;
    Ok(())
}

pub async fn stop_function_container(
    client: &Docker,
    config: &Config,
    function_name: &str,
) -> Result<()> {
    if !config.functions.contains_key(function_name) {
        return Err(CoupeError::InvalidInput(format!(
            "Function {} not found",
            function_name
        )));
    }

    let container_name = config.function_container_name(function_name);
    stop_container(client, &container_name).await
}
