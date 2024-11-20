use handler::handle;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::{ TokioIo, TokioTimer };
use opentelemetry_semantic_conventions::trace::HTTP_RESPONSE_STATUS_CODE;
use std::net::SocketAddr;
use tokio::{ net::TcpListener, task };
use coupe_lib::telemetry::Telemetry;
use tracing::{ field::{ self }, info, info_span };
use anyhow::Result;

#[tokio::main]
pub async fn main() -> Result<()> {
    Telemetry::init()?;

    // This address is localhost
    let addr: SocketAddr = ([0, 0, 0, 0], 80).into();

    // Bind to the port and listen for incoming TCP connections
    let listener = TcpListener::bind(addr).await?;
    info!("Listening on http://{}", addr);
    loop {
        let (tcp, _) = listener.accept().await?;
        let io = TokioIo::new(tcp);
        task::spawn(async move {
            if
                let Err(err) = http1::Builder
                    ::new()
                    .timer(TokioTimer::new())
                    .serve_connection(
                        io,
                        service_fn(|req| async {
                            let span = info_span!(
                                "coupe_function_execution",
                                { HTTP_RESPONSE_STATUS_CODE } = field::Empty
                            );
                            let res = span.in_scope(|| { handle(req) }).await?;
                            span.record(HTTP_RESPONSE_STATUS_CODE, &res.status().as_u16());
                            Ok::<_, anyhow::Error>(res)
                        })
                    ).await
            {
                info!("Error serving connection: {:?}", err);
            }
        });
    }
}
