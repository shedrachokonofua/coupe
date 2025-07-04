use crate::DB;
use bincode::{deserialize, serialize};
use coupe::{
    Config, CoupeError, Docker, Result, connect_docker, ensure_function_running,
    stop_function_container,
};
use fjall::{PartitionCreateOptions, PartitionHandle};
use futures::future::try_join_all;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    sync::{Arc, LazyLock},
    time::Duration,
};
use tokio::time::{Instant, sleep};
use tracing::{error, info, instrument};

static SESSION_STORE: LazyLock<PartitionHandle> = LazyLock::new(|| {
    DB.open_partition("sessions", PartitionCreateOptions::default())
        .expect("Failed to open sessions tree")
});

static DOCKER_CLIENT: LazyLock<Docker> = LazyLock::new(|| {
    connect_docker(&coupe::DeploymentTarget::Local).expect("Failed to connect to Docker")
});

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub function_name: String,
    /**
     * Nanoseconds between the UNIX epoch and when the session ends.
     */
    pub ends_at: i128,
}

impl Session {
    pub fn new(function_name: String, duration: Duration) -> Self {
        Self {
            function_name,
            ends_at: Timestamp::now().as_nanosecond() + (duration.as_nanos() as i128),
        }
    }

    pub fn is_expired(&self) -> bool {
        self.ends_at < Timestamp::now().as_nanosecond()
    }
}

impl TryInto<Value> for Session {
    type Error = CoupeError;

    fn try_into(self) -> Result<Value> {
        Ok(json!({
            "function_name": self.function_name,
            "ends_at": Timestamp::from_nanosecond(self.ends_at)
                .map_err(|e| CoupeError::DateTime(e.to_string()))?
                .to_string(),
        }))
    }
}

impl TryFrom<&[u8]> for Session {
    type Error = CoupeError;

    fn try_from(value: &[u8]) -> Result<Self> {
        deserialize(value).map_err(|e| CoupeError::Database(e.to_string()))
    }
}

impl TryInto<Vec<u8>> for Session {
    type Error = CoupeError;

    fn try_into(self) -> Result<Vec<u8>> {
        serialize(&self).map_err(|e| CoupeError::Database(e.to_string()))
    }
}

#[instrument]
pub async fn save_session(input_session: Session) -> Result<Session> {
    let existing_session = SESSION_STORE
        .get(input_session.function_name.clone())
        .map_err(|e| CoupeError::Database(e.to_string()))?;
    let mut next_session = input_session.clone();
    if let Some(existing) = existing_session {
        let existing = Session::try_from(existing.as_ref())?;
        if !existing.is_expired() && existing.ends_at > input_session.ends_at {
            next_session = existing;
        }
    }
    let next_slice: Vec<u8> = next_session.clone().try_into()?;
    SESSION_STORE
        .insert(input_session.function_name, next_slice)
        .map_err(|e| CoupeError::Database(e.to_string()))?;
    Ok(next_session)
}

#[instrument]
pub async fn delete_session(function_name: String) -> Result<()> {
    SESSION_STORE
        .remove(function_name)
        .map_err(|e| CoupeError::Database(e.to_string()))?;
    Ok(())
}

#[instrument]
pub async fn get_session(function_name: String) -> Result<Option<Session>> {
    let partition = SESSION_STORE
        .get(function_name)
        .map_err(|e| CoupeError::Database(e.to_string()))?;
    let session = partition
        .map(|session| Session::try_from(session.as_ref()))
        .transpose()
        .map_err(|e| CoupeError::Database(e.to_string()))?;
    Ok(session)
}

#[instrument]
pub async fn get_all_sessions() -> Result<Vec<Session>> {
    let mut sessions = Vec::new();
    for node in SESSION_STORE.iter() {
        if let Ok((_, session)) = node {
            sessions.push(Session::try_from(session.as_ref())?);
        }
    }
    Ok(sessions)
}

#[instrument]
pub async fn get_expired_sessions() -> Result<Vec<Session>> {
    let mut sessions = Vec::new();
    for node in SESSION_STORE.iter() {
        if let Ok((_, session)) = node {
            let session = Session::try_from(session.as_ref())?;
            if session.is_expired() {
                sessions.push(session);
            }
        }
    }
    Ok(sessions)
}

#[instrument]
pub async fn start_session(config: &Config, function_name: String) -> Result<Session> {
    let function_config =
        config
            .functions
            .get(function_name.as_str())
            .ok_or(CoupeError::InvalidInput(format!(
                "Function {} not found",
                function_name
            )))?;
    let scaling_config = function_config.scaling.clone().unwrap_or_default();
    let session_duration = Duration::from_secs(scaling_config.session_duration.unwrap_or(30));
    info!(
        function_name = function_name.as_str(),
        duration = session_duration.as_secs(),
        "Starting session"
    );
    let start = Instant::now();
    let run_result =
        ensure_function_running(&DOCKER_CLIENT, config, function_name.as_str()).await?;
    let session = save_session(Session::new(function_name.clone(), session_duration)).await?;
    let elapsed = Instant::now() - start;
    info!(
        function_name = function_name.as_str(),
        duration = elapsed.as_secs(),
        coldstarted = run_result.coldstarted,
        "Session started"
    );
    Ok(session)
}

#[instrument]
pub async fn end_session(config: &Config, function_name: String) -> Result<()> {
    info!(function_name = function_name.as_str(), "Ending session");
    stop_function_container(&DOCKER_CLIENT, config, function_name.as_str()).await?;
    delete_session(function_name).await?;
    Ok(())
}

pub async fn watch_sessions(config: Arc<Config>) -> Result<()> {
    loop {
        info!("Checking for expired sessions");
        let expired_sessions = get_expired_sessions().await?;
        info!(count = expired_sessions.len(), "Expired sessions count");

        try_join_all(
            expired_sessions
                .into_iter()
                .map(|session| {
                    let config = Arc::clone(&config);
                    async move {
                        if let Err(e) = end_session(&config, session.function_name).await {
                            error!(error = e.to_string().as_str(), "Failed to end session");
                        }
                        Ok::<_, CoupeError>(())
                    }
                })
                .collect::<Vec<_>>(),
        )
        .await?;

        sleep(Duration::from_secs(1)).await;
    }
}
