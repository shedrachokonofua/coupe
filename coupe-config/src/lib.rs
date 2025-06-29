use openapi::{Operations, Schema};
use serde::{Deserialize, Deserializer, Serialize, Serializer, ser::SerializeMap};
use std::{collections::HashMap, sync::Arc};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Broker {
    #[serde(rename = "nats")]
    Nats { connection: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum HttpAuth {
    #[serde(rename = "web")]
    Web {
        protected_routes: Vec<String>,
        policies: Vec<String>,
    },
    #[serde(rename = "jwt")]
    Jwt {
        scopes: Vec<String>,
        policies: Vec<String>,
    },
}

fn serialize_schema<T, S>(
    schema: &Option<HashMap<String, Arc<T>>>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    T: Serialize,
    S: Serializer,
{
    match schema {
        Some(map) => {
            let mut seq = serializer.serialize_map(Some(map.len()))?;
            for (k, v) in map {
                seq.serialize_entry(k, v.as_ref())?;
            }
            seq.end()
        }
        None => serializer.serialize_none(),
    }
}

fn deserialize_schema<'de, T, D>(
    deserializer: D,
) -> Result<Option<HashMap<String, Arc<T>>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    // First let Serde do its normal work
    let raw: Option<HashMap<String, T>> = Option::deserialize(deserializer)?;

    // Wrap every T value in an Arc
    Ok(raw.map(|m| {
        m.into_iter()
            .map(|(k, v)| (k, Arc::new(v)))
            .collect::<HashMap<_, _>>()
    }))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Trigger {
    #[serde(rename = "http")]
    Http {
        path: String,
        #[serde(
            default,
            serialize_with = "serialize_schema",
            deserialize_with = "deserialize_schema"
        )]
        schema: Option<HashMap<String, Arc<Operations>>>,
        auth: Option<HttpAuth>,
    },
    #[serde(rename = "queue")]
    Queue { queue: String },
    #[serde(rename = "stream")]
    Stream { stream: String },
    #[serde(rename = "timer")]
    Timer { schedule: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scaling {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_duration: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health_check_interval: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Function {
    pub image: String,
    pub trigger: Trigger,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scaling: Option<Scaling>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sentinel {
    pub port: Option<u16>,
    pub otel_endpoint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityProvider {
    #[serde(rename = "type")]
    pub provider_type: String,
    pub domain: String,
    pub client_id: String,
    pub client_secret: String,
    pub audience: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identity {
    pub provider: IdentityProvider,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Queue {
    pub broker: String,
    pub subject: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stream {
    pub broker: String,
    pub stream: String,
    pub subjects: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consumer_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenApi {
    #[serde(
        default,
        serialize_with = "serialize_schema",
        deserialize_with = "deserialize_schema"
    )]
    pub definitions: Option<HashMap<String, Arc<Schema>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sentinel: Option<Sentinel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity: Option<Identity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brokers: Option<HashMap<String, Broker>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queues: Option<HashMap<String, Queue>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub streams: Option<HashMap<String, Stream>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub openapi: Option<OpenApi>,
    #[serde(default)]
    pub functions: HashMap<String, Function>,
}
