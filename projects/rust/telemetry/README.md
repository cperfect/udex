# udex-telemetry

OpenTelemetry setup for Udex binaries — the open-standard observability boundary.

This crate is the **only** place in the workspace that configures an OpenTelemetry provider/exporter (`opentelemetry_sdk` + `opentelemetry-otlp`). Binaries (the server, the CLI) call `init` to build the OTLP exporters, sampler, and resource attributes from configuration; everything else in the workspace depends only on the vendor-neutral `tracing` API, so the application is never coupled to a specific backend.

## What it provides

- `init(&TelemetryConfig, ServiceIdentity) -> Result<TelemetryGuard, TelemetryError>` — builds one combined `tracing-subscriber`: always-on JSON-to-stdout (the durable log floor) plus optional OTLP traces, metrics, and logs when an endpoint is configured. Installs the global tracer/meter/logger providers and the W3C `TraceContextPropagator`. Returns an error if config is invalid, the OTLP CA cannot be read, or an exporter cannot be built; otherwise the returned guard flushes and shuts the providers down on drop.
- `TelemetryConfig` — the serde-deserialisable configuration contract embedded by the server/CLI as their `observability` section: `enabled`, `otlp_endpoint`, `otlp_ca`, `dangerous_allow_non_tls` (TLS is required by default; opt into a plaintext `http://` endpoint, local/dev only — mirrors the datastore/authz non-TLS flags), `sample_ratio`, per-signal toggles, `resource_attributes`, and `otlp_headers`.
- `make_request_span(method, headers)` / `record_request(method, status, elapsed)` — server-side helpers used by the gRPC middleware to create the per-request span (continuing an inbound `traceparent`) and record per-method request metrics, keeping all OpenTelemetry usage inside this crate.
- `TelemetryError` — a `thiserror` type that never exposes third-party errors.

## Bringing your own OTLP backend

The application emits **plain OTLP** and is backend-agnostic — point the server or CLI at any OTLP-compatible backend by changing the endpoint, never the code. The local dev/CI fixture (ST0027, backend replaced in ST0028) presents a keyless, plaintext collector; the collector's own credential for the backend behind it is never seen by the application. Header-authed backends (Honeycomb, Grafana Cloud, the ClickStack all-in-one) are supported directly via `otlp_headers`. Header values are commonly secrets, so they are **redacted in `Debug`** and never echoed in validation errors.

Server config:

```yaml
observability:
  enabled: true
  otlp_endpoint: "https://api.honeycomb.io:443"
  otlp_headers:
    x-honeycomb-team: "<api-key>"
```

CLI (opt-in via env; `UDEX_OTLP_HEADERS` is comma-separated `key=value`, split on the first `=` so base64 values survive):

```bash
export UDEX_OTLP_ENDPOINT="https://api.honeycomb.io:443"
export UDEX_OTLP_HEADERS="x-honeycomb-team=<api-key>"
```

**ClickStack / HyperDX all-in-one** gates ingestion behind a per-team key sent as a **raw** `authorization` header — the value is the bare key with **no `Bearer` prefix** (its bundled collector uses `bearertokenauth` with an empty scheme):

```yaml
observability:
  enabled: true
  otlp_endpoint: "http://<host>:4317"
  dangerous_allow_non_tls: true            # the all-in-one terminates no TLS
  otlp_headers:
    authorization: "<ingestion-api-key>"   # raw key, NOT "Bearer <key>"
```

`otlp_headers` exists precisely so users can plug into such backends without any application change; our own dev/CI prefers the keyless modular collector.

## Hybrid logging

JSON logs are **always** written to stdout (a durable floor that survives a Collector outage). When an OTLP endpoint is configured, logs are *also* exported over OTLP for trace/log correlation. OTLP transport uses TLS for `https://` endpoints (with `otlp_ca`); `http://` endpoints are plaintext (the local dev/CI fixture).

## Who does not use this crate

The SDK (`udex-sdk`) deliberately does **not** depend on `udex-telemetry`: as a client library it never installs a global provider. It uses only the `opentelemetry` API to emit client spans and inject `traceparent`, composing into a host application's own OpenTelemetry setup.

See [docs/ARCHITECTURE.md#observability](../../../docs/ARCHITECTURE.md#observability) for the system-level design and the [compose observability fixture](../../compose/README.md#observability) for the local backend (OpenObserve + collector + Vector).
