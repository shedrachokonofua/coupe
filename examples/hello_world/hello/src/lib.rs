use http_body_util::Full;
use hyper::body::Bytes;
use hyper::{Request, Response};
use std::convert::Infallible;

pub async fn handle(
    _: Request<hyper::body::Incoming>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    Ok(Response::new(Full::new(Bytes::from("Hello, World!"))))
}
