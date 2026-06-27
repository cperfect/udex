//! Server-side request telemetry middleware.
//!
//! A small tower layer that records a per-method request counter and latency
//! histogram for every gRPC call, then delegates to `udex-telemetry` (which owns
//! all OpenTelemetry usage). Request *spans* and W3C parent extraction are set up
//! separately via `udex_telemetry::make_request_span` on the tower-http
//! `TraceLayer` in [`crate::server`]; this layer only handles metrics so the
//! request handlers stay free of cross-cutting instrumentation.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Instant;
use tower::{Layer, Service};

/// Tower layer that records gRPC request metrics.
#[derive(Clone, Copy, Default)]
pub struct MetricsLayer;

impl<S> Layer<S> for MetricsLayer {
    type Service = MetricsService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        MetricsService { inner }
    }
}

/// Service produced by [`MetricsLayer`].
#[derive(Clone)]
pub struct MetricsService<S> {
    inner: S,
}

impl<S, ReqBody, ResBody> Service<http::Request<ReqBody>> for MetricsService<S>
where
    S: Service<http::Request<ReqBody>, Response = http::Response<ResBody>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    ReqBody: Send + 'static,
{
    type Response = http::Response<ResBody>;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: http::Request<ReqBody>) -> Self::Future {
        let method = req.uri().path().to_string();
        let start = Instant::now();

        // tower's contract: call the same instance that was poll_ready'd. Clone the
        // (ready) inner service and swap it in so the moved future owns a ready copy.
        let clone = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, clone);

        Box::pin(async move {
            let response = inner.call(req).await?;
            let code = grpc_status_code(response.headers());
            udex_telemetry::record_request(&method, code, start.elapsed());
            Ok(response)
        })
    }
}

/// Read the gRPC status code from response headers.
///
/// For unary RPCs tonic returns errors as a "trailers-only" response with the
/// `grpc-status` carried in the headers, while successful responses carry it in
/// the trailers (absent from headers). So an absent header means success (0).
fn grpc_status_code(headers: &http::HeaderMap) -> i64 {
    headers
        .get("grpc-status")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0)
}
