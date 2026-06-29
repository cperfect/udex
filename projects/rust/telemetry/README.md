# udex-telemetry

OpenTelemetry setup for Udex binaries — the open-standard observability boundary.

This crate is the **only** place in the workspace that configures an OpenTelemetry provider/exporter (`opentelemetry_sdk` + `opentelemetry-otlp`). Binaries (the server, the CLI) call `init` to build the OTLP exporters, sampler, and resource attributes from configuration; everything else in the workspace depends only on the vendor-neutral `tracing` API, so the application is never coupled to a specific backend.

## What it provides

- `init(&TelemetryConfig, ServiceIdentity) -> Result<TelemetryGuard, TelemetryError>` — builds one combined `tracing-subscriber`: always-on JSON-to-stdout (the durable log floor) plus optional OTLP traces, metrics, and logs when an endpoint is configured. Installs the global tracer/meter/logger providers and the W3C `TraceContextPropagator`. Returns an error if config is invalid, the OTLP CA cannot be read, or an exporter cannot be built; otherwise the returned guard flushes and shuts the providers down on drop.
- `TelemetryConfig` — the serde-deserialisable configuration contract embedded by the server/CLI as their `observability` section: `enabled`, `otlp_endpoint`, `otlp_ca`, `sample_ratio`, per-signal toggles, and `resource_attributes`.
- `make_request_span(method, headers)` / `record_request(method, status, elapsed)` — server-side helpers used by the gRPC middleware to create the per-request span (continuing an inbound `traceparent`) and record per-method request metrics, keeping all OpenTelemetry usage inside this crate.
- `TelemetryError` — a `thiserror` type that never exposes third-party errors.

## Hybrid logging

JSON logs are **always** written to stdout (a durable floor that survives a Collector outage). When an OTLP endpoint is configured, logs are *also* exported over OTLP for trace/log correlation. OTLP transport uses TLS.

## Who does not use this crate

The SDK (`udex-sdk`) deliberately does **not** depend on `udex-telemetry`: as a client library it never installs a global provider. It uses only the `opentelemetry` API to emit client spans and inject `traceparent`, composing into a host application's own OpenTelemetry setup.

See [docs/ARCHITECTURE.md#observability](../../../docs/ARCHITECTURE.md#observability) for the system-level design and the [observability stack](../../observability/README.md) for the local backends.
