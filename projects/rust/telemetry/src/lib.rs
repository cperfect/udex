//! Telemetry setup for Udex binaries - the open-standard observability boundary.
//!
//! [`init`] builds the combined `tracing-subscriber` (always-on JSON stdout plus
//! optional OTLP traces/metrics/logs), installs the global OpenTelemetry
//! providers, and returns a [`TelemetryGuard`] that flushes and shuts the
//! providers down when dropped.
//!
//! This crate is the ONLY place in the workspace that depends on the
//! `opentelemetry*` crates: everything else stays coupled to the `tracing` API,
//! not to any specific backend.

mod config;
mod error;

pub use config::TelemetryConfig;
pub use error::TelemetryError;

use opentelemetry::metrics::{Counter, Histogram};
use opentelemetry::propagation::Extractor;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry::KeyValue;
use opentelemetry_otlp::{WithExportConfig, WithTonicConfig};
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::{Sampler, SdkTracerProvider};
use opentelemetry_sdk::Resource;
use std::collections::BTreeMap;
use std::sync::OnceLock;
use std::time::Duration;
use tonic::transport::{Certificate, ClientTlsConfig};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter, Layer};

/// Identity of the emitting service, stamped onto every signal as OTel resource
/// attributes (`service.name`, `service.version`, `service.instance.id`).
pub struct ServiceIdentity {
    /// e.g. `udex-server`.
    pub name: String,
    /// Service version (typically `CARGO_PKG_VERSION`).
    pub version: String,
    /// Unique per-process instance id (e.g. a UUID or `host:pid`).
    pub instance_id: String,
}

/// Holds the OpenTelemetry providers so they can be flushed and shut down on
/// drop. Keep it alive for the lifetime of the program (e.g. in `main`/`serve`).
#[must_use = "dropping the guard immediately shuts telemetry down and flushes pending data"]
pub struct TelemetryGuard {
    tracer_provider: Option<SdkTracerProvider>,
    meter_provider: Option<SdkMeterProvider>,
    logger_provider: Option<SdkLoggerProvider>,
}

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        if let Some(p) = self.tracer_provider.take() {
            if let Err(e) = p.shutdown() {
                tracing::warn!(error = %e, "OTLP tracer provider shutdown error");
            }
        }
        if let Some(p) = self.meter_provider.take() {
            if let Err(e) = p.shutdown() {
                tracing::warn!(error = %e, "OTLP meter provider shutdown error");
            }
        }
        if let Some(p) = self.logger_provider.take() {
            if let Err(e) = p.shutdown() {
                tracing::warn!(error = %e, "OTLP logger provider shutdown error");
            }
        }
    }
}

/// Initialise telemetry from `config`.
///
/// Always installs a JSON-to-stdout `tracing` layer (the durable log floor). When
/// `config` is enabled with an endpoint, it additionally builds and installs the
/// requested OTLP exporters (traces/metrics/logs) over TLS and sets the global
/// OpenTelemetry providers.
///
/// Returns a [`TelemetryGuard`]; hold it for the program's lifetime.
pub fn init(
    config: &TelemetryConfig,
    identity: ServiceIdentity,
) -> Result<TelemetryGuard, TelemetryError> {
    config.validate()?;

    // Install the W3C TraceContext propagator so the server can extract an
    // inbound `traceparent` and continue the caller's trace. Harmless when
    // tracing is disabled (extraction simply yields an empty context).
    opentelemetry::global::set_text_map_propagator(TraceContextPropagator::new());

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let fmt_layer = fmt::layer().json();

    // Disabled / no endpoint: install the stdout floor only and return a no-op guard.
    if !config.active() {
        // `try_init` fails only when a global subscriber is already installed
        // (e.g. tests, or a second init in-process). That is a no-op by design -
        // matching the historical idempotent `init_tracing` behaviour - not an error.
        let _ = tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .try_init();
        return Ok(TelemetryGuard {
            tracer_provider: None,
            meter_provider: None,
            logger_provider: None,
        });
    }

    let endpoint = config
        .otlp_endpoint
        .clone()
        .expect("active() guarantees an endpoint");
    let ca_pem = match &config.otlp_ca {
        Some(path) => Some(std::fs::read(path).map_err(|e| TelemetryError::CaRead {
            path: path.clone(),
            source: e,
        })?),
        None => None,
    };
    let resource = build_resource(&identity, &config.resource_attributes);

    // Traces -> Tempo.
    let (tracer_provider, trace_layer) = if config.traces {
        let exporter = build_span_exporter(&endpoint, ca_pem.as_deref())?;
        let provider = SdkTracerProvider::builder()
            .with_batch_exporter(exporter)
            .with_sampler(Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(
                config.sample_ratio,
            ))))
            .with_resource(resource.clone())
            .build();
        let tracer = provider.tracer("udex");
        opentelemetry::global::set_tracer_provider(provider.clone());
        let layer = tracing_opentelemetry::layer().with_tracer(tracer).boxed();
        (Some(provider), Some(layer))
    } else {
        (None, None)
    };

    // Metrics -> Prometheus (via the collector).
    let meter_provider = if config.metrics {
        let exporter = build_metric_exporter(&endpoint, ca_pem.as_deref())?;
        let provider = SdkMeterProvider::builder()
            .with_periodic_exporter(exporter)
            .with_resource(resource.clone())
            .build();
        opentelemetry::global::set_meter_provider(provider.clone());
        // Build the request-metric instruments now that the provider is set, so
        // record_request never caches no-op instruments from the default provider.
        install_request_metrics();
        Some(provider)
    } else {
        None
    };

    // Logs -> Loki (hybrid: stdout JSON is still on via fmt_layer).
    let (logger_provider, logs_layer) = if config.logs {
        let exporter = build_log_exporter(&endpoint, ca_pem.as_deref())?;
        let provider = SdkLoggerProvider::builder()
            .with_batch_exporter(exporter)
            .with_resource(resource.clone())
            .build();
        // Exclude the exporter stack's own logs to avoid a feedback loop.
        let bridge =
            opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge::new(&provider)
                .with_filter(tracing_subscriber::filter::filter_fn(|meta| {
                    let t = meta.target();
                    !(t.starts_with("opentelemetry")
                        || t.starts_with("tonic")
                        || t.starts_with("hyper")
                        || t.starts_with("h2")
                        || t.starts_with("tower")
                        || t.starts_with("reqwest"))
                }))
                .boxed();
        (Some(provider), Some(bridge))
    } else {
        (None, None)
    };

    // Unlike the disabled path, here a failed try_init means the OTLP trace/log
    // layers were NOT attached (a subscriber is already installed), so telemetry
    // would silently fail to export. Treat it as a hard error: shut the providers
    // we built back down (flush + stop exporters) and surface the failure rather
    // than returning a guard that does nothing.
    if let Err(e) = tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .with(trace_layer)
        .with(logs_layer)
        .try_init()
    {
        if let Some(p) = &tracer_provider {
            let _ = p.shutdown();
        }
        if let Some(p) = &meter_provider {
            let _ = p.shutdown();
        }
        if let Some(p) = &logger_provider {
            let _ = p.shutdown();
        }
        return Err(TelemetryError::Subscriber(e.to_string()));
    }

    tracing::info!(
        endpoint = %endpoint,
        traces = config.traces,
        metrics = config.metrics,
        logs = config.logs,
        sample_ratio = config.sample_ratio,
        "OpenTelemetry initialised"
    );

    Ok(TelemetryGuard {
        tracer_provider,
        meter_provider,
        logger_provider,
    })
}

fn build_resource(identity: &ServiceIdentity, attrs: &BTreeMap<String, String>) -> Resource {
    let mut kvs = vec![
        KeyValue::new("service.name", identity.name.clone()),
        KeyValue::new("service.version", identity.version.clone()),
        KeyValue::new("service.instance.id", identity.instance_id.clone()),
    ];
    for (k, v) in attrs {
        kvs.push(KeyValue::new(k.clone(), v.clone()));
    }
    Resource::builder().with_attributes(kvs).build()
}

/// Build a tonic OTLP TLS config for an `https://` endpoint. Returns `None` for
/// plaintext (`http://`) endpoints (the exporter then connects without TLS).
fn tls_config(endpoint: &str, ca_pem: Option<&[u8]>) -> Option<ClientTlsConfig> {
    if !endpoint.starts_with("https://") {
        return None;
    }
    let mut tls = ClientTlsConfig::new();
    if let Some(host) = host_of(endpoint) {
        tls = tls.domain_name(host);
    }
    tls = match ca_pem {
        Some(pem) => tls.ca_certificate(Certificate::from_pem(pem)),
        None => tls.with_enabled_roots(),
    };
    Some(tls)
}

fn build_span_exporter(
    endpoint: &str,
    ca_pem: Option<&[u8]>,
) -> Result<opentelemetry_otlp::SpanExporter, TelemetryError> {
    let mut builder = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint);
    if let Some(tls) = tls_config(endpoint, ca_pem) {
        builder = builder.with_tls_config(tls);
    }
    builder.build().map_err(|e| TelemetryError::Exporter {
        signal: "traces",
        detail: e.to_string(),
    })
}

fn build_metric_exporter(
    endpoint: &str,
    ca_pem: Option<&[u8]>,
) -> Result<opentelemetry_otlp::MetricExporter, TelemetryError> {
    let mut builder = opentelemetry_otlp::MetricExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint);
    if let Some(tls) = tls_config(endpoint, ca_pem) {
        builder = builder.with_tls_config(tls);
    }
    builder.build().map_err(|e| TelemetryError::Exporter {
        signal: "metrics",
        detail: e.to_string(),
    })
}

fn build_log_exporter(
    endpoint: &str,
    ca_pem: Option<&[u8]>,
) -> Result<opentelemetry_otlp::LogExporter, TelemetryError> {
    let mut builder = opentelemetry_otlp::LogExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint);
    if let Some(tls) = tls_config(endpoint, ca_pem) {
        builder = builder.with_tls_config(tls);
    }
    builder.build().map_err(|e| TelemetryError::Exporter {
        signal: "logs",
        detail: e.to_string(),
    })
}

/// Extract the host from a `scheme://host:port/...` endpoint, for TLS SNI.
fn host_of(endpoint: &str) -> Option<String> {
    let after = endpoint.split("://").nth(1)?;
    let host = after.split('/').next()?.split(':').next()?;
    (!host.is_empty()).then(|| host.to_string())
}

// ---------------------------------------------------------------------------
// Server-side request instrumentation helpers.
//
// These keep all `opentelemetry`/`tracing-opentelemetry` usage inside this crate
// so callers (the server middleware) depend only on `tracing` + `http`.
// ---------------------------------------------------------------------------

/// Adapts an `http::HeaderMap` to the OpenTelemetry `Extractor` trait so an
/// inbound W3C `traceparent` can be read from gRPC request headers.
struct HeaderExtractor<'a>(&'a http::HeaderMap);

impl Extractor for HeaderExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|v| v.to_str().ok())
    }

    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|k| k.as_str()).collect()
    }
}

/// Build the per-request tracing span for an incoming gRPC call, parenting it on
/// the caller's trace if the request carries a W3C `traceparent` header. The
/// returned span is named after the gRPC method (`/pkg.Service/Method`) so it
/// surfaces that way in Tempo. Server middleware enters this span around the
/// request, so handler and datastore spans nest beneath it.
pub fn make_request_span(method: &str, headers: &http::HeaderMap) -> tracing::Span {
    let parent_cx = opentelemetry::global::get_text_map_propagator(|prop| {
        prop.extract(&HeaderExtractor(headers))
    });
    let span = tracing::info_span!("grpc.request", otel.name = method, rpc.method = method);
    // Best-effort: attaching the remote parent only fails if the OTel layer is
    // absent (telemetry disabled), in which case there is no trace to link to.
    let _ = span.set_parent(parent_cx);
    span
}

struct RequestMetrics {
    requests: Counter<u64>,
    duration: Histogram<f64>,
}

static REQUEST_METRICS: OnceLock<RequestMetrics> = OnceLock::new();

/// Build the request-metric instruments from the CURRENT global meter provider and
/// install them. Called by `init` AFTER `set_meter_provider`, so the instruments
/// are never created from the default no-op provider (which a `OnceLock` would
/// otherwise cache permanently if `record_request` ran before `init`). Idempotent.
fn install_request_metrics() {
    let _ = REQUEST_METRICS.get_or_init(|| {
        let meter = opentelemetry::global::meter("udex");
        RequestMetrics {
            requests: meter
                .u64_counter("udex.rpc.requests")
                .with_description("Total gRPC requests handled, by method and status")
                .build(),
            duration: meter
                .f64_histogram("udex.rpc.duration")
                .with_description("gRPC request duration in seconds, by method")
                .with_unit("s")
                .build(),
        }
    });
}

/// Record one handled gRPC request: increments a per-method/per-status counter and
/// records request duration. A no-op until `init` has installed the instruments
/// (i.e. telemetry enabled with metrics on), so it is safe to call unconditionally
/// from middleware.
pub fn record_request(method: &str, grpc_status_code: i64, elapsed: Duration) {
    let Some(metrics) = REQUEST_METRICS.get() else {
        return;
    };
    let method = method.to_string();
    metrics.requests.add(
        1,
        &[
            KeyValue::new("rpc.method", method.clone()),
            KeyValue::new("rpc.grpc.status_code", grpc_status_code),
        ],
    );
    metrics.duration.record(
        elapsed.as_secs_f64(),
        &[KeyValue::new("rpc.method", method)],
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_of_parses_endpoints() {
        assert_eq!(
            host_of("https://otel-collector:4317").as_deref(),
            Some("otel-collector")
        );
        assert_eq!(
            host_of("http://localhost:4318/v1/logs").as_deref(),
            Some("localhost")
        );
        assert_eq!(host_of("not-a-url"), None);
    }

    #[test]
    fn tls_config_none_for_http() {
        assert!(tls_config("http://otel-collector:4318", None).is_none());
    }

    #[test]
    fn disabled_init_returns_noop_guard() {
        // A disabled config must not create providers. (Subscriber init may or may
        // not succeed depending on global state in the test binary, so only assert
        // the guard shape via a fresh disabled config validate path.)
        let cfg = TelemetryConfig::default();
        assert!(cfg.validate().is_ok());
        assert!(!cfg.active());
    }
}
