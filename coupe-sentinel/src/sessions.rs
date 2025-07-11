use crate::DB;
use bincode::{deserialize, serialize};
use coupe::{
    Config, CoupeError, Docker, Result, connect_docker, ensure_function_running,
    stop_function_container,
};
use dashmap::DashMap;
use fjall::{PartitionCreateOptions, TransactionalPartitionHandle};
use futures::future::try_join_all;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fmt::Display,
    sync::{Arc, LazyLock},
    time::Duration,
};
use tokio::{
    sync::Mutex as TokioMutex,
    time::{Instant, sleep, timeout},
};
use tracing::{debug, error, info, instrument};

static SESSION_STORE: LazyLock<TransactionalPartitionHandle> = LazyLock::new(|| {
    DB.open_partition("sessions", PartitionCreateOptions::default())
        .expect("Failed to open sessions tree")
});

pub static DOCKER_CLIENT: LazyLock<Docker> = LazyLock::new(|| {
    connect_docker(&coupe::DeploymentTarget::Local).expect("Failed to connect to Docker")
});

type FunctionLock = Arc<TokioMutex<()>>;

static FUNCTION_LOCKS: LazyLock<DashMap<String, FunctionLock>> = LazyLock::new(|| DashMap::new());

async fn get_function_lock(function_name: &str) -> FunctionLock {
    FUNCTION_LOCKS
        .entry(function_name.to_string())
        .or_insert_with(|| Arc::new(TokioMutex::new(())))
        .clone()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub function_name: String,
    /**
     * Nanoseconds between the UNIX epoch and when the session ends.
     */
    pub ends_at: i128,
}

impl Display for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Session(function_name={}, ends_at={})",
            self.function_name, self.ends_at
        )
    }
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
    let mut tx = DB.write_tx();
    let existing_session = tx
        .get(&SESSION_STORE, input_session.function_name.clone())
        .map_err(|e| CoupeError::Database(e.to_string()))?;
    let mut next_session = input_session.clone();
    if let Some(existing) = existing_session {
        let existing = Session::try_from(existing.as_ref())?;
        if !existing.is_expired() {
            if existing.ends_at > input_session.ends_at {
                next_session.ends_at = existing.ends_at;
            }
        }
    }
    info!(
        function_name = %input_session.function_name,
        session = %next_session,
        "Saving session"
    );
    let next_slice: Vec<u8> = next_session.clone().try_into()?;
    tx.insert(&SESSION_STORE, input_session.function_name, next_slice);
    tx.commit()
        .map_err(|e| CoupeError::Database(e.to_string()))?;
    Ok(next_session)
}

#[instrument]
pub async fn delete_session(function_name: String) -> Result<()> {
    let mut tx = DB.write_tx();
    tx.remove(&SESSION_STORE, function_name);
    tx.commit()
        .map_err(|e| CoupeError::Database(e.to_string()))?;
    Ok(())
}

#[instrument]
pub async fn get_session(function_name: String) -> Result<Option<Session>> {
    let tx = DB.read_tx();
    let session = tx
        .get(&SESSION_STORE, function_name)
        .map_err(|e| CoupeError::Database(e.to_string()))?;
    let session = session
        .map(|session| Session::try_from(session.as_ref()))
        .transpose()
        .map_err(|e| CoupeError::Database(e.to_string()))?;
    Ok(session)
}

#[instrument]
pub async fn get_all_sessions() -> Result<Vec<Session>> {
    let mut sessions = Vec::new();
    for node in DB.read_tx().iter(&SESSION_STORE) {
        if let Ok((_, session)) = node {
            sessions.push(Session::try_from(session.as_ref())?);
        }
    }
    Ok(sessions)
}

#[instrument]
pub async fn get_expired_sessions() -> Result<Vec<Session>> {
    let mut sessions = Vec::new();
    for node in DB.read_tx().iter(&SESSION_STORE) {
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
    let function_lock = get_function_lock(&function_name).await;
    let _lock_guard = function_lock.lock().await;

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
        function_name = %function_name,
        duration = session_duration.as_secs(),
        "Starting session"
    );
    let start = Instant::now();
    let run_result =
        ensure_function_running(&DOCKER_CLIENT, config, function_name.as_str()).await?;
    let session = save_session(Session::new(function_name.clone(), session_duration)).await?;
    if run_result.coldstarted {
        wait_for_healthcheck(&config.internal_function_healthcheck_url(function_name.as_str())?)
            .await?;
    }
    let elapsed = Instant::now() - start;
    info!(
        function_name = %function_name,
        duration = elapsed.as_secs(),
        coldstarted = run_result.coldstarted,
        "Session started"
    );

    Ok(session)
}

async fn wait_for_healthcheck(url: &str) -> Result<()> {
    let client = reqwest::Client::new();
    let total_timeout = Duration::from_secs(15);
    let retry_delay = Duration::from_millis(200);
    let request_timeout = Duration::from_secs(2);

    info!(
        url = %url,
        total_timeout_secs = total_timeout.as_secs(),
        retry_delay_ms = retry_delay.as_millis(),
        request_timeout_secs = request_timeout.as_secs(),
        "Starting healthcheck"
    );

    let start_time = Instant::now();
    let mut attempt_count = 0;

    let result = timeout(total_timeout, async {
        loop {
            attempt_count += 1;
            let attempt_start = Instant::now();

            info!(
                url = %url,
                attempt = attempt_count,
                elapsed_ms = start_time.elapsed().as_millis(),
                "Healthcheck attempt"
            );

            match client.get(url).timeout(request_timeout).send().await {
                Ok(response) => {
                    let status = response.status();
                    let attempt_duration = attempt_start.elapsed();

                    if status.is_success() {
                        info!(
                            url = %url,
                            attempt = attempt_count,
                            status = status.as_u16(),
                            attempt_duration_ms = attempt_duration.as_millis(),
                            total_duration_ms = start_time.elapsed().as_millis(),
                            "Healthcheck successful"
                        );
                        return;
                    } else {
                        info!(
                            url = %url,
                            attempt = attempt_count,
                            status = status.as_u16(),
                            attempt_duration_ms = attempt_duration.as_millis(),
                            elapsed_ms = start_time.elapsed().as_millis(),
                            "Healthcheck failed, retrying"
                        );
                    }
                }
                Err(e) => {
                    let attempt_duration = attempt_start.elapsed();
                    info!(
                        url = %url,
                        attempt = attempt_count,
                        error = %e,
                        attempt_duration_ms = attempt_duration.as_millis(),
                        elapsed_ms = start_time.elapsed().as_millis(),
                        "Healthcheck request failed, retrying"
                    );
                }
            }

            sleep(retry_delay).await;
        }
    })
    .await;

    match result {
        Ok(()) => {
            info!(
                url = %url,
                attempts = attempt_count,
                total_duration_ms = start_time.elapsed().as_millis(),
                "Healthcheck completed successfully"
            );
            Ok(())
        }
        Err(_) => {
            error!(
                url = %url,
                attempts = attempt_count,
                total_duration_ms = start_time.elapsed().as_millis(),
                timeout_secs = total_timeout.as_secs(),
                "Healthcheck timed out"
            );
            Err(CoupeError::Healthcheck("Healthcheck timeout".to_string()))
        }
    }
}

#[instrument]
pub async fn end_session(config: &Config, function_name: String) -> Result<()> {
    let function_lock = get_function_lock(&function_name).await;
    let _lock_guard = function_lock.lock().await;

    info!(function_name = %function_name, "Ending session");

    delete_session(function_name.clone()).await?;

    stop_function_container(&DOCKER_CLIENT, config, function_name.as_str()).await?;

    Ok(())
}

pub async fn watch_sessions(config: Arc<Config>) -> Result<()> {
    loop {
        debug!("Checking for expired sessions");
        let expired_sessions = get_expired_sessions().await?;
        debug!(count = expired_sessions.len(), "Expired sessions found");

        try_join_all(
            expired_sessions
                .into_iter()
                .map(|session| {
                    let config = Arc::clone(&config);
                    async move {
                        if let Err(e) = end_session(&config, session.function_name.clone()).await {
                            error!(error = %e, "Failed to end session");
                        }
                        info!(function_name = %session.function_name, "Session ended");
                        Ok::<_, CoupeError>(())
                    }
                })
                .collect::<Vec<_>>(),
        )
        .await?;

        sleep(Duration::from_secs(1)).await;
    }
}
