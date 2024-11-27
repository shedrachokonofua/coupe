use opentelemetry::{ global::{ self, BoxedTracer }, KeyValue };
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::{ LogExporter, SpanExporter, WithExportConfig };
use opentelemetry::trace::TracerProvider as _;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::layer::SubscriberExt;
use opentelemetry_sdk::{
    logs::LoggerProvider,
    propagation::TraceContextPropagator,
    runtime::Tokio,
    trace::{ Config, Tracer, TracerProvider },
    Resource,
};
use tracing_subscriber::{ prelude::*, EnvFilter, Registry };
use anyhow::Result;
use std::env;

pub struct Telemetry {
    service_name: String,
}

pub struct TelemetryConfig {
    pub otel_endpoint: String,
    pub service_name: String,
    pub container_name: String,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            otel_endpoint: env
                ::var("OTEL_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:4317".to_string()),
            service_name: env::var("FUNCTION_NAME").expect("FUNCTION_NAME is required"),
            container_name: env::var("CONTAINER_NAME").expect("CONTAINER_NAME is required"),
        }
    }
}

impl Telemetry {
    pub fn init(config: TelemetryConfig) -> Result<Self> {
        let resource = Resource::new(
            vec![
                KeyValue::new(
                    opentelemetry_semantic_conventions::resource::SERVICE_NAME,
                    config.service_name.clone()
                ),
                KeyValue::new(
                    opentelemetry_semantic_conventions::resource::CONTAINER_NAME,
                    config.container_name.clone()
                )
            ]
        );
        global::set_text_map_propagator(TraceContextPropagator::new());

        let tracer = Self::init_tracer(
            &config.otel_endpoint,
            &config.service_name,
            resource.clone()
        )?;
        let logger_provider = Self::init_logger_provider(&config.otel_endpoint, resource)?;
        Self::init_tracing_subscriber(tracer, logger_provider)?;

        Ok(Self {
            service_name: config.service_name,
        })
    }

    fn init_tracer(endpoint: &str, service_name: &str, resource: Resource) -> Result<Tracer> {
        let exporter = SpanExporter::builder().with_tonic().with_endpoint(endpoint).build()?;

        let provider = TracerProvider::builder()
            .with_batch_exporter(exporter, Tokio)
            .with_config(Config::default().with_resource(resource))
            .build();

        let tracer = provider.tracer(Self::tracer_name(service_name));

        global::set_tracer_provider(provider);

        Ok(tracer)
    }

    fn init_logger_provider(endpoint: &str, resource: Resource) -> Result<LoggerProvider> {
        let exporter = LogExporter::builder().with_tonic().with_endpoint(endpoint).build()?;
        let logger_provider = LoggerProvider::builder()
            .with_batch_exporter(exporter, Tokio)
            .with_resource(resource)
            .build();

        Ok(logger_provider)
    }

    fn init_tracing_subscriber(tracer: Tracer, logger_provider: LoggerProvider) -> Result<()> {
        let tracing_bridge = OpenTelemetryTracingBridge::new(&logger_provider);
        let filter = EnvFilter::new("info")
            .add_directive("hyper=error".parse()?)
            .add_directive("tonic=error".parse()?)
            .add_directive("h2=error".parse()?)
            .add_directive("tower=error".parse()?)
            .add_directive("reqwest=error".parse()?)
            .add_directive("otel::tracing=trace".parse()?)
            .add_directive("otel=debug".parse()?);
        Registry::default()
            .with(filter)
            .with(OpenTelemetryLayer::new(tracer).with_error_events_to_exceptions(true))
            .with(tracing_bridge)
            .init();
        Ok(())
    }

    fn tracer_name(service_name: &str) -> String {
        format!("coupe/{}", service_name)
    }

    pub fn tracer(&self) -> BoxedTracer {
        global::tracer(Self::tracer_name(&self.service_name))
    }

    pub fn shutdown(self) -> Result<()> {
        global::shutdown_tracer_provider();
        Ok(())
    }
}
