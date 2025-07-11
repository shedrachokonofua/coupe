use crate::{Config, CoupeError, DeploymentTarget, Result, deployment_path, fluentbit_path};
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
use tracing::{debug, error, info};

const DEFAULT_SENTINEL_IMAGE: &str = "coupe/sentinel:latest";

pub fn connect_docker(target: &DeploymentTarget) -> Result<Docker> {
    info!(target = ?target, "Connecting to Docker");

    let result = match target {
        DeploymentTarget::Local => {
            debug!("Using local Docker connection");
            Docker::connect_with_unix_defaults()
        }
        DeploymentTarget::Remote(host) => {
            info!(host = %host, "Connecting to remote Docker host");
            Docker::connect_with_ssh(host, 30, API_DEFAULT_VERSION)
        }
    };

    match result {
        Ok(docker) => {
            info!(target = ?target, "Docker connection established");
            Ok(docker)
        }
        Err(e) => {
            error!(target = ?target, error = %e, "Failed to connect to Docker");
            Err(CoupeError::Docker(e.to_string()))
        }
    }
}

pub async fn create_fluentbit_container(client: &Docker, config: &Config) -> Result<()> {
    let container_name = config.fluentbit_container_name();
    let network_name = config.stack_network_name();

    info!(
        container_name = %container_name,
        network_name = %network_name,
        port = config.fluentbit_port(),
        "Creating Fluent Bit container"
    );

    let container_config = ContainerCreateBody {
        image: Some("fluent/fluent-bit:latest".to_string()),
        exposed_ports: Some(HashMap::from([(
            format!("{}/tcp", config.fluentbit_port()),
            HashMap::<(), ()>::new(),
        )])),
        host_config: Some(bollard::models::HostConfig {
            restart_policy: Some(bollard::models::RestartPolicy {
                name: Some(bollard::models::RestartPolicyNameEnum::ALWAYS),
                ..Default::default()
            }),
            network_mode: Some(network_name),
            binds: Some(vec![
                format!(
                    "{}:/fluent-bit/etc/fluent-bit.yaml:ro",
                    fluentbit_path(config).display()
                ),
                "/var/run/docker.sock:/var/run/docker.sock".to_string(),
                "/var/lib/docker/containers:/var/lib/docker/containers:ro".to_string(),
            ]),
            port_bindings: Some(HashMap::from([(
                format!("{}/tcp", config.fluentbit_port()),
                Some(vec![PortBinding {
                    host_ip: Some("0.0.0.0".to_string()),
                    host_port: Some(config.fluentbit_port().to_string()),
                }]),
            )])),
            ..Default::default()
        }),
        cmd: Some(vec![
            "/fluent-bit/bin/fluent-bit".to_string(),
            "-c".to_string(),
            "/fluent-bit/etc/fluent-bit.yaml".to_string(),
        ]),
        ..Default::default()
    };

    let options = CreateContainerOptionsBuilder::new()
        .name(&container_name)
        .build();

    match client
        .create_container(Some(options), container_config)
        .await
    {
        Ok(_) => {
            info!(container_name = %container_name, "Fluent Bit container created successfully");
            Ok(())
        }
        Err(e) => {
            error!(container_name = %container_name, error = %e, "Failed to create Fluent Bit container");
            Err(CoupeError::Docker(e.to_string()))
        }
    }
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

    info!(
        container_name = %container_name,
        network_name = %network_name,
        image = %sentinel_image,
        port = config.sentinel_port(),
        "Creating Sentinel container"
    );

    let container_config = ContainerCreateBody {
        image: Some(sentinel_image.clone()),
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
            restart_policy: Some(bollard::models::RestartPolicy {
                name: Some(bollard::models::RestartPolicyNameEnum::ALWAYS),
                ..Default::default()
            }),
            network_mode: Some(network_name),
            binds: Some(vec![
                format!("{}:/usr/app:rw", deployment_path(config).display()),
                "/var/run/docker.sock:/var/run/docker.sock".to_string(),
            ]),
            port_bindings: Some(HashMap::from([(
                format!("{}/tcp", config.sentinel_port()),
                Some(vec![PortBinding {
                    host_ip: Some("0.0.0.0".to_string()),
                    host_port: Some(config.sentinel_port().to_string()),
                }]),
            )])),
            log_config: Some(bollard::models::HostConfigLogConfig {
                typ: Some("fluentd".to_string()),
                config: Some(HashMap::from([
                    (
                        "fluentd-address".to_string(),
                        format!("localhost:{}", config.fluentbit_port()),
                    ),
                    ("tag".to_string(), config.sentinel_container_name()),
                    ("fluentd-async".to_string(), "true".to_string()),
                ])),
            }),
            ..Default::default()
        }),
        ..Default::default()
    };

    let options = CreateContainerOptionsBuilder::new()
        .name(&container_name)
        .build();

    match client
        .create_container(Some(options), container_config)
        .await
    {
        Ok(_) => {
            info!(container_name = %container_name, image = %sentinel_image, "Sentinel container created successfully");
            Ok(())
        }
        Err(e) => {
            error!(container_name = %container_name, image = %sentinel_image, error = %e, "Failed to create Sentinel container");
            Err(CoupeError::Docker(e.to_string()))
        }
    }
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

    info!(
        function_name = %function_name,
        container_name = %container_name,
        network_name = %network_name,
        image = %function_config.image,
        "Creating function container"
    );

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
            log_config: Some(bollard::models::HostConfigLogConfig {
                typ: Some("fluentd".to_string()),
                config: Some(HashMap::from([
                    (
                        "fluentd-address".to_string(),
                        format!("localhost:{}", config.fluentbit_port()),
                    ),
                    ("tag".to_string(), container_name.clone()),
                    ("fluentd-async".to_string(), "true".to_string()),
                ])),
            }),
            ..Default::default()
        }),
        ..Default::default()
    };

    let options = CreateContainerOptionsBuilder::new()
        .name(&container_name)
        .build();

    match client
        .create_container(Some(options), container_config)
        .await
    {
        Ok(_) => {
            info!(
                function_name = %function_name,
                container_name = %container_name,
                image = %function_config.image,
                "Function container created successfully"
            );
            Ok(())
        }
        Err(e) => {
            error!(
                function_name = %function_name,
                container_name = %container_name,
                image = %function_config.image,
                error = %e,
                "Failed to create function container"
            );
            Err(CoupeError::Docker(e.to_string()))
        }
    }
}

pub async fn create_containers(client: &Docker, config: &Config) -> Result<()> {
    info!(stack_name = %config.name, "Creating all containers");

    create_fluentbit_container(client, config).await?;
    create_sentinel_container(client, config).await?;

    for name in config.functions.keys() {
        create_function_container(client, config, name).await?;
    }

    info!(stack_name = %config.name, "All containers created successfully");
    Ok(())
}

pub async fn create_network(client: &Docker, config: &Config) -> Result<()> {
    let network_name = config.stack_network_name();

    info!(
        stack_name = %config.name,
        network_name = %network_name,
        "Creating Docker network"
    );

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

    match client.create_network(options).await {
        Ok(_) => {
            info!(network_name = %network_name, "Docker network created successfully");
            Ok(())
        }
        Err(e) => {
            error!(network_name = %network_name, error = %e, "Failed to create Docker network");
            Err(CoupeError::Docker(e.to_string()))
        }
    }
}

pub async fn get_container_status(
    client: &Docker,
    container_id: &str,
) -> Result<ContainerStateStatusEnum> {
    debug!(container_id = %container_id, "Getting container status");

    let inspect_result = client
        .inspect_container(container_id, None::<InspectContainerOptions>)
        .await
        .map_err(|e| {
            error!(container_id = %container_id, error = %e, "Failed to inspect container");
            CoupeError::Docker(e.to_string())
        })?;

    let status = inspect_result
        .state
        .and_then(|state| state.status)
        .unwrap_or(ContainerStateStatusEnum::EMPTY);

    debug!(container_id = %container_id, status = ?status, "Container status retrieved");
    Ok(status)
}

pub struct ContainerRunResult {
    pub coldstarted: bool,
}

async fn ensure_container_running(
    client: &Docker,
    container_id: &str,
) -> Result<ContainerRunResult> {
    info!(container_id = %container_id, "Ensuring container is running");

    let status = get_container_status(client, container_id).await?;
    info!(container_id = %container_id, status = ?status, "Current container status");

    let coldstarted = match status {
        ContainerStateStatusEnum::RUNNING => {
            info!(container_id = %container_id, "Container already running");
            false
        }
        ContainerStateStatusEnum::CREATED | ContainerStateStatusEnum::EXITED => {
            info!(container_id = %container_id, "Starting container");

            client
                .start_container(container_id, None::<StartContainerOptions>)
                .await
                .map_err(|e| {
                    error!(container_id = %container_id, error = %e, "Failed to start container");
                    CoupeError::Docker(e.to_string())
                })?;

            poll_until_running(client, container_id).await?;
            info!(container_id = %container_id, "Container started successfully");
            true
        }
        ContainerStateStatusEnum::PAUSED => {
            info!(container_id = %container_id, "Unpausing container");

            client.unpause_container(container_id).await.map_err(|e| {
                error!(container_id = %container_id, error = %e, "Failed to unpause container");
                CoupeError::Docker(e.to_string())
            })?;

            poll_until_running(client, container_id).await?;
            info!(container_id = %container_id, "Container unpaused successfully");
            true
        }
        ContainerStateStatusEnum::RESTARTING => {
            info!(container_id = %container_id, "Waiting for container to finish restarting");
            poll_until_running(client, container_id).await?;
            info!(container_id = %container_id, "Container restarted successfully");
            true
        }
        _ => {
            error!(container_id = %container_id, status = ?status, "Container is in unrecoverable state");
            return Err(CoupeError::Docker(format!(
                "Container {} is in unrecoverable state: {:?}",
                container_id, status
            )));
        }
    };

    info!(container_id = %container_id, coldstarted = coldstarted, "Container is now running");
    Ok(ContainerRunResult { coldstarted })
}

pub async fn ensure_fluentbit_running(
    client: &Docker,
    config: &Config,
) -> Result<ContainerRunResult> {
    let container_name = config.fluentbit_container_name();
    info!(container_name = %container_name, "Ensuring Fluent Bit is running");
    ensure_container_running(client, &container_name).await
}

pub async fn ensure_sentinel_running(
    client: &Docker,
    config: &Config,
) -> Result<ContainerRunResult> {
    let container_name = config.sentinel_container_name();
    info!(container_name = %container_name, "Ensuring Sentinel is running");
    ensure_container_running(client, &container_name).await
}

pub async fn ensure_function_running(
    client: &Docker,
    config: &Config,
    function_name: &str,
) -> Result<ContainerRunResult> {
    let container_name = config.function_container_name(function_name);
    info!(
        function_name = %function_name,
        container_name = %container_name,
        "Ensuring function container is running"
    );
    ensure_container_running(client, &container_name).await
}

pub async fn recreate_docker_stack(config: &Config, target: &DeploymentTarget) -> Result<()> {
    info!(
        stack_name = %config.name,
        target = ?target,
        "Recreating Docker stack"
    );

    let client = connect_docker(target)?;
    teardown(&client, config).await?;
    create_network(&client, config).await?;
    create_containers(&client, config).await?;
    ensure_fluentbit_running(&client, config).await?;
    ensure_sentinel_running(&client, config).await?;

    info!(stack_name = %config.name, "Docker stack recreated successfully");
    Ok(())
}

async fn poll_until_running(client: &Docker, container_id: &str) -> Result<()> {
    let timeout = Duration::from_secs(30);
    let interval = Duration::from_millis(500);
    let start_time = Instant::now();

    debug!(
        container_id = %container_id,
        timeout_secs = timeout.as_secs(),
        "Polling container until running"
    );

    while start_time.elapsed() < timeout {
        let status = get_container_status(client, container_id).await?;

        if status == ContainerStateStatusEnum::RUNNING {
            info!(
                container_id = %container_id,
                elapsed_ms = start_time.elapsed().as_millis(),
                "Container is now running"
            );
            return Ok(());
        }

        debug!(
            container_id = %container_id,
            status = ?status,
            elapsed_ms = start_time.elapsed().as_millis(),
            "Container not running yet, continuing to poll"
        );

        sleep(interval).await;
    }

    error!(
        container_id = %container_id,
        timeout_secs = timeout.as_secs(),
        "Container failed to start within timeout"
    );

    Err(CoupeError::Docker(format!(
        "Container {} failed to start within timeout",
        container_id
    )))
}

pub async fn teardown(client: &Docker, config: &Config) -> Result<()> {
    info!(stack_name = %config.name, "Tearing down Docker stack");

    for name in config.functions.keys() {
        let container_name = config.function_container_name(name);
        info!(container_name = %container_name, "Removing function container");
        remove_container_if_exists(client, &container_name).await?;
    }

    let sentinel_container_name = config.sentinel_container_name();
    info!(container_name = %sentinel_container_name, "Removing Sentinel container");
    remove_container_if_exists(client, &sentinel_container_name).await?;

    let fluentbit_container_name = config.fluentbit_container_name();
    info!(container_name = %fluentbit_container_name, "Removing Fluent Bit container");
    remove_container_if_exists(client, &fluentbit_container_name).await?;

    let network_name = config.stack_network_name();
    info!(network_name = %network_name, "Removing Docker network");
    remove_network_if_exists(client, &network_name).await?;

    info!(stack_name = %config.name, "Docker stack teardown completed");
    Ok(())
}

async fn remove_container_if_exists(client: &Docker, container_name: &str) -> Result<()> {
    info!(container_name = %container_name, "Removing container if exists");

    let options = RemoveContainerOptionsBuilder::new().force(true).build();

    match client.remove_container(container_name, Some(options)).await {
        Ok(_) => {
            info!(container_name = %container_name, "Container removed successfully");
            Ok(())
        }
        Err(e) => {
            if let BollardError::DockerResponseServerError { status_code, .. } = &e {
                if *status_code == 404 {
                    debug!(container_name = %container_name, "Container not found (already removed)");
                    return Ok(());
                }
            }
            error!(container_name = %container_name, error = %e, "Failed to remove container");
            Err(CoupeError::Docker(e.to_string()))
        }
    }
}

async fn remove_network_if_exists(client: &Docker, network_name: &str) -> Result<()> {
    info!(network_name = %network_name, "Removing network if exists");

    match client.remove_network(network_name).await {
        Ok(_) => {
            info!(network_name = %network_name, "Network removed successfully");
            Ok(())
        }
        Err(e) => {
            if let BollardError::DockerResponseServerError { status_code, .. } = &e {
                if *status_code == 404 {
                    debug!(network_name = %network_name, "Network not found (already removed)");
                    return Ok(());
                }
            }
            error!(network_name = %network_name, error = %e, "Failed to remove network");
            Err(CoupeError::Docker(e.to_string()))
        }
    }
}

async fn stop_container(client: &Docker, container_name: &str) -> Result<()> {
    info!(container_name = %container_name, "Stopping container");

    match client
        .stop_container(container_name, None::<StopContainerOptions>)
        .await
    {
        Ok(_) => {
            info!(container_name = %container_name, "Container stopped successfully");
            Ok(())
        }
        Err(e) => {
            error!(container_name = %container_name, error = %e, "Failed to stop container");
            Err(CoupeError::Docker(e.to_string()))
        }
    }
}

pub async fn stop_function_container(
    client: &Docker,
    config: &Config,
    function_name: &str,
) -> Result<()> {
    if !config.functions.contains_key(function_name) {
        error!(function_name = %function_name, "Function not found in config");
        return Err(CoupeError::InvalidInput(format!(
            "Function {} not found",
            function_name
        )));
    }

    let container_name = config.function_container_name(function_name);
    info!(
        function_name = %function_name,
        container_name = %container_name,
        "Stopping function container"
    );

    stop_container(client, &container_name).await
}
