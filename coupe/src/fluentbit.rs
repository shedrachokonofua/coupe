use crate::{Config, Result};
use serde_json::json;

const OTEL_RESOURCE_MAPPER: &str = r#"
function process(tag, timestamp, record)
  -- Create or update resource attributes
  if not record["resource"] then
    record["resource"] = {}
  end
  if not record["resource"]["attributes"] then
    record["resource"]["attributes"] = {}
  end
  -- Set service.name to the tag
  record["resource"]["attributes"]["service.name"] = tag
  return 1, timestamp, record
end"#;

fn parse_otel_endpoint(endpoint: &str) -> Result<(String, u16)> {
    let endpoint = endpoint.trim();

    if let Some(colon_pos) = endpoint.rfind(':') {
        let host = &endpoint[..colon_pos];
        let port_str = &endpoint[colon_pos + 1..];

        let port = port_str.parse::<u16>().map_err(|_| {
            crate::CoupeError::InvalidInput(format!("Invalid port in otel_endpoint: {}", port_str))
        })?;

        Ok((host.to_string(), port))
    } else {
        Ok((endpoint.to_string(), 4318))
    }
}

pub fn build_fluentbit_config(stack_config: &Config) -> Result<serde_yaml::Value> {
    let use_otel = stack_config
        .sentinel
        .as_ref()
        .and_then(|s| s.otel_endpoint.as_ref())
        .is_some();

    let outputs = match stack_config
        .sentinel
        .as_ref()
        .and_then(|s| s.otel_endpoint.as_ref())
    {
        Some(otel_endpoint) => {
            let (host, port) = parse_otel_endpoint(otel_endpoint)?;

            vec![json!({
                "name": "opentelemetry",
                "match": "*",
                "host": host,
                "port": port,
                "logs_uri": "/v1/logs",
                "log_response_payload": true,
                "tls": "off",
                "tls.verify": "off"
            })]
        }
        None => {
            vec![json!({
                "name": "stdout",
                "match": "*"
            })]
        }
    };

    let mut forward_input = json!({
        "name": "forward",
        "listen": "0.0.0.0",
        "port": stack_config.fluentbit_port(),
        "buffer_chunk_size": "1M",
        "buffer_max_size": "6M"
    });

    if use_otel {
        forward_input["processors"] = json!({
            "logs": [
                {
                    "name": "opentelemetry_envelope"
                },
                {
                    "name": "lua",
                    "code": OTEL_RESOURCE_MAPPER,
                    "call": "process"
                }
            ]
        });
    }

    let json_value = json!({
      "service": {
        "flush": 5,
        "daemon": "off",
        "log_level": "info",
        "http_server": "on",
        "http_listen": "0.0.0.0",
        "http_port": 2020,
      },
      "pipeline": {
        "inputs": [
          forward_input,
          {
            "name": "docker_events",
            "unix_path": "/var/run/docker.sock",
            "tag": stack_config.sentinel_container_name(),
          },
        ],
        "outputs": outputs,
      }
    });

    Ok(serde_yaml::to_value(json_value)?)
}
