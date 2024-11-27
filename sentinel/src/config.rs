use once_cell::sync::Lazy;
use serde::{ Deserialize, Serialize };
use config::Config;
use clap::{ command, Parser };
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[arg(long, required = false)]
    pub nats_url: Option<String>,

    #[arg(long, required = false, default_value = "./coupe.yaml")]
    pub config: PathBuf,
}

pub static ARGS: Lazy<Args> = Lazy::new(|| Args::parse());

#[derive(Debug, Serialize, Deserialize)]
pub struct Queue {
    pub name: String,
    pub subjects: Vec<String>,
    pub max_age_secs: Option<u64>,
    pub max_num_messages: Option<u64>,
    pub duplicate_window_secs: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Stream {
    pub name: String,
    pub subjects: Vec<String>,
    pub max_age_secs: Option<u64>,
    pub max_num_messages: Option<u64>,
    pub duplicate_window_secs: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Trigger {
    #[serde(rename = "http")] Http {
        route: String,
    },
    #[serde(rename = "pubsub")] PubSub {
        subjects: Vec<String>,
    },
    #[serde(rename = "queue")] Queue {
        name: String,
    },
    #[serde(rename = "stream")] Stream {
        name: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Function {
    pub name: String,
    pub runtime: String,
    pub session_duration: Option<u64>,
    pub trigger: Trigger,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CoupeConfig {
    pub name: String,
    pub http_port: u16,
    pub otel_endpoint: String,
    pub functions: Vec<Function>,
    #[serde(default)]
    pub queues: Vec<Queue>,
    #[serde(default)]
    pub streams: Vec<Stream>,
}

impl CoupeConfig {
    pub fn sentinel_container_name(&self) -> String {
        format!("coupe_stack_{}_sentinel", self.name)
    }

    pub fn function_container_name(&self, function_name: &str) -> String {
        format!("coupe_function_{}_{}", self.name, function_name)
    }
}

pub static CONFIG: Lazy<CoupeConfig> = Lazy::new(|| {
    let config = Config::builder()
        .add_source(config::File::from(ARGS.config.clone()))
        .build()
        .expect("Failed to build config");

    config.try_deserialize::<CoupeConfig>().expect("Failed to parse config")
});
