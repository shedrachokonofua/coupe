use std::time::Duration;
use bollard::{ secret::ContainerStateStatusEnum, Docker };
use once_cell::sync::Lazy;
use anyhow::Result;
use tokio::time::Instant;
use tracing::{ info, info_span, instrument, Instrument };

use crate::{ config::CONFIG, error::SentinelError };

static DOCKER: Lazy<Docker> = Lazy::new(|| {
    Docker::connect_with_local_defaults().expect("Failed to connect to Docker")
});

#[instrument]
async fn get_container_status(function_name: String) -> Result<Option<ContainerStateStatusEnum>> {
    let containers = DOCKER.inspect_container(
        &CONFIG.function_container_name(&function_name),
        None
    ).await.map_err(|err| {
        if err.to_string().contains("status code 404") {
            SentinelError::ContainerNotFound(function_name.clone())
        } else {
            SentinelError::ContainerDaemonRequestFailed(err.to_string())
        }
    })?;
    Ok(containers.state.and_then(|state| state.status))
}

#[derive(Debug, Clone)]
pub struct PollConfig {
    timeout: Duration,
    interval: Duration,
}

impl Default for PollConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            interval: Duration::from_micros(50),
        }
    }
}

#[instrument]
async fn poll_until_container_running(name: &str, poll_config: PollConfig) -> Result<()> {
    let start_time = Instant::now();

    while start_time.elapsed() < poll_config.timeout {
        let status = get_container_status(name.to_string()).await?.ok_or_else(||
            SentinelError::ContainerNotFound(name.to_string())
        )?;

        info!(status = status.to_string(), function_name = name, "Polled container status");

        if status == ContainerStateStatusEnum::RUNNING {
            return Ok(());
        }
        tokio::time::sleep(poll_config.interval).await;
    }
    Err(SentinelError::ContainerStartupTimeout(name.to_string()).into())
}

#[instrument]
pub async fn ensure_container_running(
    function_name: String,
    poll_config: PollConfig
) -> Result<()> {
    let status = get_container_status(function_name.clone()).await?.ok_or_else(||
        SentinelError::ContainerNotFound(function_name.clone())
    )?;

    match status {
        ContainerStateStatusEnum::RUNNING => Ok(()),
        ContainerStateStatusEnum::EMPTY | ContainerStateStatusEnum::REMOVING => {
            Err(SentinelError::ContainerNotFound(function_name).into())
        }
        ContainerStateStatusEnum::DEAD => {
            Err(SentinelError::ContainerNotRecoverable(function_name).into())
        }
        ContainerStateStatusEnum::CREATED | ContainerStateStatusEnum::EXITED => {
            DOCKER.start_container::<String>(
                &CONFIG.function_container_name(&function_name),
                None
            ).instrument(info_span!("start_container")).await?;
            poll_until_container_running(&function_name, poll_config).await
        }
        ContainerStateStatusEnum::PAUSED => {
            DOCKER.unpause_container(&CONFIG.function_container_name(&function_name)).instrument(
                info_span!("unpause_container")
            ).await?;
            poll_until_container_running(&function_name, poll_config).await
        }
        ContainerStateStatusEnum::RESTARTING => {
            poll_until_container_running(&function_name, poll_config).await
        }
    }
}

#[instrument]
pub async fn stop_container(function_name: String) -> Result<()> {
    DOCKER.stop_container(&CONFIG.function_container_name(&function_name), None).await?;
    Ok(())
}
