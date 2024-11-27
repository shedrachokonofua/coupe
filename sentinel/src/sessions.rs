use std::{ collections::HashMap, time::Duration };
use coupe_lib::{ metrics::CoupeFunctionMetrics, telemetry::{ Telemetry, TelemetryConfig } };
use fjall::{ PartitionCreateOptions, PartitionHandle };
use futures::future::try_join_all;
use named_lock::NamedLock;
use once_cell::sync::Lazy;
use serde::{ Deserialize, Serialize };
use serde_json::{ json, Value };
use tokio::time::{ sleep, Instant };
use bincode::{ serialize, deserialize };
use anyhow::Result;
use tracing::{ error, info, instrument };
use jiff::Timestamp;
use crate::{
    config::CONFIG,
    containers::{ ensure_container_running, stop_container, PollConfig },
    db::DB,
};
use opentelemetry::metrics::MeterProvider;

static SESSION_STORE: Lazy<PartitionHandle> = Lazy::new(|| {
    DB.open_partition("sessions", PartitionCreateOptions::default()).expect(
        "Failed to open sessions tree"
    )
});

static FUNCTION_METRICS: Lazy<HashMap<String, CoupeFunctionMetrics>> = Lazy::new(|| {
    let mut function_metrics = HashMap::new();
    for function in &CONFIG.functions {
        let function_name = function.name.clone();
        let function_telemetry_config = TelemetryConfig {
            otel_endpoint: CONFIG.otel_endpoint.clone(),
            service_name: function_name.clone(),
            container_name: CONFIG.function_container_name(&function_name),
        };
        let metric_provider = Telemetry::init_metrics_provider(
            &function_telemetry_config.otel_endpoint.clone(),
            function_telemetry_config.into()
        ).expect("Failed to initialize metrics provider");
        let meter = metric_provider.meter("coupe/sentinel");
        function_metrics.insert(function_name.clone(), CoupeFunctionMetrics::new(meter));
    }
    function_metrics
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
    type Error = anyhow::Error;

    fn try_into(self) -> Result<Value> {
        Ok(
            json!({
                "function_name": self.function_name,
                "ends_at": Timestamp::from_nanosecond(self.ends_at)?.to_string(),
            })
        )
    }
}

impl TryFrom<&[u8]> for Session {
    type Error = anyhow::Error;

    fn try_from(value: &[u8]) -> Result<Self> {
        deserialize(value).map_err(Into::into)
    }
}

impl TryInto<Vec<u8>> for Session {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<Vec<u8>> {
        serialize(&self).map_err(Into::into)
    }
}

#[instrument]
pub async fn save_session(input_session: Session) -> Result<Session> {
    let lock = NamedLock::create(&format!("session:{}", input_session.function_name.clone()))?;
    let _guard = lock.lock()?;

    let existing_session = SESSION_STORE.get(input_session.function_name.clone())?;
    let mut next_session = input_session.clone();
    if let Some(existing) = existing_session {
        let existing = Session::try_from(existing.as_ref())?;
        if !existing.is_expired() && existing.ends_at > input_session.ends_at {
            next_session = existing;
        }
    }
    let next_slice: Vec<u8> = next_session.clone().try_into()?;
    SESSION_STORE.insert(input_session.function_name, next_slice)?;
    Ok(next_session)
}

#[instrument]
pub async fn delete_session(function_name: String) -> Result<()> {
    SESSION_STORE.remove(function_name)?;
    Ok(())
}

#[instrument]
pub async fn get_session(function_name: String) -> Result<Option<Session>> {
    let session = SESSION_STORE.get(function_name)?
        .map(|session| Session::try_from(session.as_ref()))
        .transpose()?;
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
async fn record_function_init(
    function_name: String,
    duration: Duration,
    is_cold_start: bool
) -> Result<()> {
    let function_metrics = FUNCTION_METRICS.get(&function_name).expect(
        "Function metrics not found"
    );
    function_metrics.record_init(duration, is_cold_start, &[]);
    Ok(())
}

#[instrument]
pub async fn start_session(
    function_name: String,
    session_duration: Duration,
    status_poll_config: PollConfig
) -> Result<Session> {
    info!(
        function_name = function_name.as_str(),
        duration = session_duration.as_secs(),
        "Starting session"
    );
    let start = Instant::now();
    let coldstarted = ensure_container_running(function_name.clone(), status_poll_config).await?;
    let session = save_session(Session::new(function_name.clone(), session_duration)).await?;
    let elapsed = Instant::now() - start;
    record_function_init(function_name, elapsed, coldstarted).await?;
    Ok(session)
}

#[instrument]
pub async fn end_session(function_name: String) -> Result<()> {
    info!(function_name = function_name.as_str(), "Ending session");
    stop_container(function_name.clone()).await?;
    delete_session(function_name).await?;
    Ok(())
}

pub async fn watch_sessions() -> Result<()> {
    loop {
        info!("Checking for expired sessions");
        let expired_sessions = get_expired_sessions().await?;
        info!(count = expired_sessions.len(), "Expired sessions count");

        try_join_all(
            expired_sessions
                .into_iter()
                .map(|session| async move {
                    if let Err(e) = end_session(session.function_name).await {
                        error!(error = e.to_string().as_str(), "Failed to end session");
                    }
                    Ok::<_, anyhow::Error>(())
                })
                .collect::<Vec<_>>()
        ).await?;

        sleep(Duration::from_secs(1)).await;
    }
}
