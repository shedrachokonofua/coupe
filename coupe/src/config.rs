use crate::{CoupeError, Result};
use openapi::{Operations, Schema};
use serde::{Deserialize, Deserializer, Serialize, Serializer, ser::SerializeMap};
use std::{collections::HashMap, fs, path::PathBuf, sync::Arc};

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
    let raw: Option<HashMap<String, T>> = Option::deserialize(deserializer)?;

    Ok(raw.map(|m| {
        m.into_iter()
            .map(|(k, v)| (k, Arc::new(v)))
            .collect::<HashMap<_, _>>()
    }))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HttpMethod {
    Any,
    Get,
    Post,
    Put,
    Delete,
    Patch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Trigger {
    #[serde(rename = "http")]
    Http {
        path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        method: Option<HttpMethod>,
        #[serde(
            default,
            serialize_with = "serialize_schema",
            deserialize_with = "deserialize_schema",
            skip_serializing_if = "Option::is_none"
        )]
        schema: Option<HashMap<String, Arc<Operations>>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        auth: Option<HttpAuth>,
    },
    #[serde(rename = "queue")]
    Queue { queue: String },
    #[serde(rename = "stream")]
    Stream { stream: String },
    #[serde(rename = "timer")]
    Timer { schedule: String },
}

impl Trigger {
    pub fn as_http(
        self,
    ) -> Option<(
        String,
        Option<HttpMethod>,
        Option<HashMap<String, Arc<Operations>>>,
        Option<HttpAuth>,
    )> {
        if let Trigger::Http {
            path,
            method,
            schema,
            auth,
        } = self
        {
            Some((path, method, schema, auth))
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handler_port: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerRegistry {
    pub url: String,
    pub namespace: Option<String>,
}

pub const DEFAULT_SENTINEL_PORT: u16 = 52345;
pub const DEFAULT_FUNCTION_HANDLER_PORT: u16 = 80;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Sentinel {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registry: Option<ContainerRegistry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
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

impl Config {
    pub fn load(path: PathBuf) -> Result<Config> {
        if !path.exists() {
            return Err(CoupeError::InvalidInput(format!(
                "Config file not found: {}",
                path.display()
            )));
        }

        let config_content = fs::read_to_string(&path).map_err(|e| CoupeError::Io(e))?;
        let config: Config =
            serde_yaml::from_str(&config_content).map_err(|e| CoupeError::Yaml(e))?;

        Ok(config)
    }

    pub fn to_yaml(&self) -> Result<String> {
        let yaml = serde_yaml::to_string(self).map_err(|e| CoupeError::Yaml(e))?;
        Ok(yaml)
    }

    pub fn stack_network_name(&self) -> String {
        format!("coupe-{}-network", self.name)
    }

    pub fn sentinel_container_name(&self) -> String {
        format!("coupe-{}-sentinel", self.name)
    }

    pub fn function_container_name(&self, function_name: &str) -> String {
        format!("coupe-{}-function-{}", self.name, function_name)
    }

    pub fn sentinel_port(&self) -> u16 {
        self.sentinel
            .as_ref()
            .and_then(|s| s.port)
            .unwrap_or(DEFAULT_SENTINEL_PORT)
    }

    pub fn http_functions(&self) -> Vec<String> {
        self.functions
            .iter()
            .filter_map(|(name, func)| {
                if let Trigger::Http { .. } = &func.trigger {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn internal_function_url(&self, function_name: &str) -> Result<String> {
        let function_url = format!(
            "{}:{}",
            self.function_container_name(function_name),
            self.function_handler_port(function_name)?
        );
        Ok(function_url)
    }

    pub fn function_handler_port(&self, function_name: &str) -> Result<u16> {
        let function = self
            .functions
            .get(function_name)
            .ok_or(CoupeError::InvalidInput(format!(
                "Function {} not found",
                function_name
            )))?;
        Ok(function
            .handler_port
            .unwrap_or(DEFAULT_FUNCTION_HANDLER_PORT))
    }

    pub fn internal_function_healthcheck_url(&self, function_name: &str) -> Result<String> {
        Ok(format!(
            "http://{}/health",
            self.internal_function_url(function_name)?
        ))
    }
}
