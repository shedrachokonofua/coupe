use crate::{Config, ConfigTarget, CoupeError, DeploymentTarget, Result};
use bollard::API_DEFAULT_VERSION;
use bollard::errors::Error as BollardError;
use bollard::models::{ContainerCreateBody, ContainerStateStatusEnum, NetworkCreateRequest};
use bollard::query_parameters::{
    CreateContainerOptionsBuilder, InspectContainerOptions, RemoveContainerOptionsBuilder,
    StartContainerOptions,
};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::{Instant, sleep};

pub use bollard::Docker;

pub struct StackDockerClient<'a> {
    client: Docker,
    config: &'a Config,
    config_target: ConfigTarget<'a>,
}

const DEFAULT_SENTINEL_IMAGE: &str = "coupe/sentinel:latest";

impl<'a> StackDockerClient<'a> {
    pub fn new(config: &'a Config, target: &'a DeploymentTarget) -> Result<Self> {
        let client = match &target {
            DeploymentTarget::Local => Docker::connect_with_unix_defaults(),
            DeploymentTarget::Remote(host) => {
                Docker::connect_with_ssh(host, 30, API_DEFAULT_VERSION)
            }
        }
        .map_err(|e| CoupeError::Docker(e.to_string()))?;

        let config_target = ConfigTarget::new(config, target);

        Ok(Self {
            client,
            config,
            config_target,
        })
    }

    pub async fn create_sentinel_container(&self) -> Result<()> {
        let container_name = self.config.sentinel_container_name();
        let network_name = self.config.stack_network_name();

        self.config_target.deploy().await?;

        let sentinel_image = self
            .config
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

        let bind_mount = format!("{}:/usr/app:ro", self.config_target.dir_path().display());

        let container_config = ContainerCreateBody {
            image: Some(sentinel_image),
            env: Some(vec![format!("COUPE_STACK={}", self.config.name)]),
            labels: Some({
                let mut labels = HashMap::new();
                labels.insert("coupe.stack".to_string(), self.config.name.clone());
                labels.insert("coupe.role".to_string(), "sentinel".to_string());
                labels
            }),
            host_config: Some(bollard::models::HostConfig {
                network_mode: Some(network_name),
                binds: Some(vec![bind_mount]),
                ..Default::default()
            }),
            ..Default::default()
        };

        let options = CreateContainerOptionsBuilder::new()
            .name(&container_name)
            .build();

        self.client
            .create_container(Some(options), container_config)
            .await
            .map_err(|e| CoupeError::Docker(e.to_string()))?;

        Ok(())
    }

    pub async fn create_function_container(&self, function_name: String) -> Result<()> {
        let function_config =
            self.config.functions.get(&function_name).ok_or_else(|| {
                CoupeError::Config(format!("Function {} not found", function_name))
            })?;

        let container_name = self.config.function_container_name(&function_name);
        let network_name = self.config.stack_network_name();

        let container_config = ContainerCreateBody {
            image: Some(function_config.image.clone()),
            env: Some(vec![
                format!("COUPE_STACK={}", self.config.name),
                format!("COUPE_FUNCTION={}", function_name),
            ]),
            labels: Some({
                let mut labels = HashMap::new();
                labels.insert("coupe.stack".to_string(), self.config.name.clone());
                labels.insert("coupe.role".to_string(), "function".to_string());
                labels.insert("coupe.function".to_string(), function_name.clone());
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

        self.client
            .create_container(Some(options), container_config)
            .await
            .map_err(|e| CoupeError::Docker(e.to_string()))?;

        Ok(())
    }

    pub async fn create_containers(&self) -> Result<()> {
        self.create_sentinel_container().await?;
        for (name, _) in &self.config.functions {
            self.create_function_container(name.clone()).await?;
        }
        Ok(())
    }

    pub async fn create_network(&self) -> Result<()> {
        let network_name = self.config.stack_network_name();

        let options = NetworkCreateRequest {
            name: network_name.clone(),
            driver: Some("bridge".to_string()),
            labels: Some({
                let mut labels = HashMap::new();
                labels.insert("coupe.stack".to_string(), self.config.name.clone());
                labels
            }),
            ..Default::default()
        };

        self.client
            .create_network(options)
            .await
            .map_err(|e| CoupeError::Docker(e.to_string()))?;

        Ok(())
    }

    pub async fn ensure_container_running(&self, container_id: &str) -> Result<()> {
        let inspect_result = self
            .client
            .inspect_container(container_id, None::<InspectContainerOptions>)
            .await
            .map_err(|e| CoupeError::Docker(e.to_string()))?;

        let status = inspect_result
            .state
            .and_then(|state| state.status)
            .unwrap_or(ContainerStateStatusEnum::EMPTY);

        match status {
            ContainerStateStatusEnum::RUNNING => Ok(()),
            ContainerStateStatusEnum::CREATED | ContainerStateStatusEnum::EXITED => {
                self.client
                    .start_container(container_id, None::<StartContainerOptions>)
                    .await
                    .map_err(|e| CoupeError::Docker(e.to_string()))?;

                self.poll_until_running(container_id).await
            }
            ContainerStateStatusEnum::PAUSED => {
                self.client
                    .unpause_container(container_id)
                    .await
                    .map_err(|e| CoupeError::Docker(e.to_string()))?;

                self.poll_until_running(container_id).await
            }
            ContainerStateStatusEnum::RESTARTING => self.poll_until_running(container_id).await,
            _ => Err(CoupeError::Docker(format!(
                "Container {} is in unrecoverable state: {:?}",
                container_id, status
            ))),
        }
    }

    pub async fn ensure_sentinel_running(&self) -> Result<()> {
        let container_name = self.config.sentinel_container_name();
        self.ensure_container_running(&container_name).await
    }

    async fn poll_until_running(&self, container_id: &str) -> Result<()> {
        let timeout = Duration::from_secs(30);
        let interval = Duration::from_millis(500);
        let start_time = Instant::now();

        while start_time.elapsed() < timeout {
            let inspect_result = self
                .client
                .inspect_container(container_id, None::<InspectContainerOptions>)
                .await
                .map_err(|e| CoupeError::Docker(e.to_string()))?;

            let status = inspect_result
                .state
                .and_then(|state| state.status)
                .unwrap_or(ContainerStateStatusEnum::EMPTY);

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

    pub async fn teardown(&self) -> Result<()> {
        println!("Tearing down stack");
        for (name, _) in &self.config.functions {
            let container_name = self.config.function_container_name(name);
            println!("Removing container {}", container_name);
            self.remove_container_if_exists(&container_name).await?;
        }

        let sentinel_container_name = self.config.sentinel_container_name();
        self.remove_container_if_exists(&sentinel_container_name)
            .await?;

        let network_name = self.config.stack_network_name();
        self.remove_network_if_exists(&network_name).await?;

        self.config_target.remove().await?;

        Ok(())
    }

    async fn remove_container_if_exists(&self, container_name: &str) -> Result<()> {
        let options = RemoveContainerOptionsBuilder::new().force(true).build();

        match self
            .client
            .remove_container(container_name, Some(options))
            .await
        {
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

    async fn remove_network_if_exists(&self, network_name: &str) -> Result<()> {
        match self.client.remove_network(network_name).await {
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
}
