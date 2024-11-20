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

impl Telemetry {
    pub fn init() -> Result<Self> {
        let endpoint = env::var("OTEL_ENDPOINT")?;
        let function_name = env::var("FUNCTION_NAME")?;
        let container_name = env::var("CONTAINER_NAME")?;

        let resource = Resource::new(
            vec![
                KeyValue::new(
                    opentelemetry_semantic_conventions::resource::SERVICE_NAME,
                    container_name.clone()
                ),
                KeyValue::new(
                    opentelemetry_semantic_conventions::resource::FAAS_NAME,
                    function_name.clone()
                ),
                KeyValue::new(
                    opentelemetry_semantic_conventions::resource::CONTAINER_NAME,
                    container_name.clone()
                )
            ]
        );
        global::set_text_map_propagator(TraceContextPropagator::new());

        let tracer = Self::init_tracer(&endpoint, &function_name, resource.clone())?;
        let logger_provider = Self::init_logger_provider(&endpoint, resource)?;
        Self::init_tracing_subscriber(tracer, logger_provider)?;

        Ok(Self {
            service_name: function_name,
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
        let filter = EnvFilter::new("debug")
            .add_directive("hyper=error".parse()?)
            .add_directive("tonic=error".parse()?)
            .add_directive("h2=error".parse()?)
            .add_directive("tower=error".parse()?)
            .add_directive("reqwest=error".parse()?);
        Registry::default()
            .with(filter)
            .with(OpenTelemetryLayer::new(tracer))
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
