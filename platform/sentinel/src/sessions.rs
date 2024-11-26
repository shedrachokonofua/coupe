use std::time::Duration;
use futures::future::try_join_all;
use once_cell::sync::Lazy;
use serde::{ Deserialize, Serialize };
use serde_json::{ json, Value };
use sled::{ IVec, Tree };
use tokio::time::sleep;
use bincode::{ serialize, deserialize };
use anyhow::Result;
use tracing::{ error, info, instrument };
use jiff::Timestamp;
use crate::{ containers::{ ensure_container_running, stop_container, PollConfig }, db::DB };

static SESSION_STORE: Lazy<Tree> = Lazy::new(|| {
    DB.open_tree("sessions").expect("Failed to open sessions tree")
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

impl TryFrom<IVec> for Session {
    type Error = anyhow::Error;

    fn try_from(value: IVec) -> Result<Self> {
        deserialize(&value).map_err(Into::into)
    }
}

impl TryInto<IVec> for Session {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<IVec> {
        serialize(&self).map(IVec::from).map_err(Into::into)
    }
}

#[instrument]
pub async fn save_session(input_session: Session) -> Result<Session> {
    let new_session = SESSION_STORE.update_and_fetch(
        input_session.function_name.clone(),
        |current: Option<&[u8]>| -> Option<Vec<u8>> {
            let current_session = current.and_then(|current|
                Session::try_from(current)
                    .inspect_err(|e| {
                        error!(error = e.to_string().as_str(), "Failed to deserialize session")
                    })
                    .ok()
            );
            let next = match current_session {
                Some(existing) if
                    !existing.is_expired() &&
                    existing.ends_at > input_session.ends_at
                => {
                    existing
                }
                _ => {
                    info!(
                        function_name = input_session.function_name.as_str(),
                        ends_at = input_session.ends_at,
                        "Saving session"
                    );
                    input_session.clone()
                }
            };
            next.try_into()
                .inspect_err(|e: &anyhow::Error| {
                    error!(error = e.to_string().as_str(), "Failed to serialize session")
                })
                .ok()
        }
    )?
        .ok_or_else(|| anyhow::anyhow!("Failed to put session"))?
        .try_into()?;
    Ok(new_session)
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
    ensure_container_running(function_name.clone(), status_poll_config).await?;
    save_session(Session::new(function_name, session_duration)).await
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
