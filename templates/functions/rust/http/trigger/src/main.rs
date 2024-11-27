use handler::handle;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::{ TokioIo, TokioTimer };
use opentelemetry::{ global::{ self, get_text_map_propagator }, KeyValue };
use opentelemetry_http::{ HeaderExtractor, HeaderInjector };
use opentelemetry_semantic_conventions::trace::{
    HTTP_REQUEST_METHOD,
    HTTP_RESPONSE_STATUS_CODE,
    HTTP_ROUTE,
};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use std::{ net::SocketAddr, sync::Arc };
use tokio::{ net::TcpListener, task, time::Instant };
use coupe_lib::{ metrics::CoupeFunctionMetrics, telemetry::{ Telemetry, TelemetryConfig } };
use tracing::{ field::{ self }, info, info_span, Instrument };
use anyhow::Result;

#[tokio::main]
pub async fn main() -> Result<()> {
    Telemetry::init(TelemetryConfig::default())?;
    let meter = global::meter("coupe/trigger");
    let function_metrics = Arc::new(CoupeFunctionMetrics::new(meter));
    // This address is localhost
    let addr: SocketAddr = ([0, 0, 0, 0], 80).into();

    // Bind to the port and listen for incoming TCP connections
    let listener = TcpListener::bind(addr).await?;
    info!("Listening on http://{}", addr);
    loop {
        let (tcp, _) = listener.accept().await?;
        let io = TokioIo::new(tcp);
        let function_metrics = function_metrics.clone();
        task::spawn(async move {
            if
                let Err(err) = http1::Builder
                    ::new()
                    .timer(TokioTimer::new())
                    .serve_connection(
                        io,
                        service_fn(|req| async {
                            let path = req.uri().path().to_string();
                            let method = req.method().as_str().to_string();

                            let span = info_span!(
                                "coupe.function_execution",
                                otel.kind = "SERVER",
                                otel.status_code = "OK",
                                { HTTP_ROUTE } = path.clone(),
                                { HTTP_REQUEST_METHOD } = method.clone(),
                                { HTTP_RESPONSE_STATUS_CODE } = field::Empty
                            );
                            span.set_parent(
                                get_text_map_propagator(|propagator| {
                                    propagator.extract(&HeaderExtractor(&req.headers()))
                                })
                            );

                            let metric_tags = vec![
                                KeyValue::new(HTTP_ROUTE, path.clone()),
                                KeyValue::new(HTTP_REQUEST_METHOD, method.clone())
                            ];

                            let start = Instant::now();
                            function_metrics.record_begin_invoke(&metric_tags);

                            let mut res = handle(req).instrument(span.clone()).await?;

                            let duration = start.elapsed();
                            function_metrics.record_end_invoke(
                                duration,
                                res.status().is_server_error(),
                                &metric_tags
                            );

                            span.record(HTTP_RESPONSE_STATUS_CODE, &res.status().as_u16());
                            if res.status().is_server_error() {
                                span.record("otel.status_code", "ERROR");
                            }

                            get_text_map_propagator(|propagator| {
                                propagator.inject_context(
                                    &span.context(),
                                    &mut HeaderInjector(res.headers_mut())
                                );
                            });

                            Ok::<_, anyhow::Error>(res)
                        })
                    ).await
            {
                info!("Error serving connection: {:?}", err);
            }
        });
    }
}
