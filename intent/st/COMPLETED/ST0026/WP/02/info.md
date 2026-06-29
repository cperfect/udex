---
verblock: "24 Jun 2026:v0.1: vscode - Initial version"
wp_id: WP-02
title: "Telemetry foundation and server enablement"
scope: Small
status: Done
---

# WP-02: Telemetry foundation and server enablement

## Objective

Introduce the open-standard telemetry boundary and turn it on in the server:
create the `udex-telemetry` crate (OTel SDK/exporter setup for binaries), add the
`observability` config section, and wire OTLP traces + metrics + hybrid logs into
server startup with graceful enable/disable and sampling.

## Deliverables

- `udex-telemetry` crate (registered in `intent/llm/MODULES.md` first - Highlander):
  - Provider construction for traces (`opentelemetry-otlp`), metrics, and logs.
  - Hybrid logging: `tracing-subscriber` JSON stdout layer always on; OTLP log
    layer (`opentelemetry-appender-tracing`) added when an endpoint is configured.
  - Resource attributes (`service.name`, `service.version`, instance id),
    configurable head sampling, OTLP-over-TLS using the configured CA.
  - Graceful shutdown/flush; typed `TelemetryError` (thiserror, no third-party
    leakage).
- `observability` config section on `ServerConfig` (serde/YAML): `enabled`,
  `otlp_endpoint`, `otlp_ca`, `sample_ratio`, `traces`/`metrics`/`logs` toggles,
  `resource_attributes`. Defaults to disabled.
- Validation in `ServerConfig::validate()` (ratio range, endpoint/CA checks,
  flags) + config test fixtures.
- Server startup wiring: initialise telemetry from config, replacing/extending the
  current `init_tracing()` path; no-op cleanly when disabled.

## Acceptance Criteria

- [x] With `observability.enabled: false` (or unset), the server behaves exactly
  as today (JSON stdout logs only; no OTLP; no Collector dependency).
- [x] With the WP01 stack up and `observability` configured, server traces appear
  in Tempo, server metrics in Prometheus, and OTLP logs in Loki - with JSON logs
  still on stdout.
- [x] Invalid config (bad ratio, unreadable CA, malformed endpoint) is rejected
  by `validate()` with a clear typed error (No Silent Errors).
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test` pass.

## As-built notes

- **New crate `udex-telemetry`** (`projects/rust/telemetry/`), registered in
  `MODULES.md`. It is the only workspace crate that depends on `opentelemetry*`.
  Public API: `init(&TelemetryConfig, ServiceIdentity) -> Result<TelemetryGuard,
  TelemetryError>`, plus the `TelemetryConfig` contract and `TelemetryError`.
- **OTel stack**: opentelemetry 0.32 / opentelemetry_sdk 0.32 (rt-tokio) /
  opentelemetry-otlp 0.32 / opentelemetry-appender-tracing 0.32 /
  tracing-opentelemetry 0.33. OTLP exporter uses **gRPC (tonic)** with a direct
  `tonic = 0.14` dep solely to build the client TLS config (custom CA). This is a
  second tonic version alongside the app's 0.13; they never cross.
- **Combined subscriber**: `init` builds one `tracing-subscriber` registry -
  always-on JSON stdout (the floor) plus optional OTel trace + OTLP-log layers.
  This subsumes the old `logging::init_tracing()` (removed; `logging.rs` keeps
  only `init_test_tracing`). The OTLP-log layer filters out the exporter stack's
  own targets to avoid a feedback loop.
- **Idempotent init**: an already-installed subscriber is a no-op (matches the
  historical behaviour), so the many in-process `serve()` calls in the test suites
  do not fail on the second server.
- **Config threading**: `observability: Option<TelemetryConfig>` added to both the
  server `ServerConfig` and the CLI `ServerConfig`, threaded through
  `UdexConfig::into_configs`. Validated in `ServerConfig::validate()` and unit
  tested (telemetry config validation; CLI YAML parse of the `observability`
  section).
- **Verified live** (WP01 stack up): a real `udex_telemetry::init` over OTLP/TLS
  emitted a tagged span -> Tempo, a counter -> Prometheus, and a log line -> Loki,
  while the same log was simultaneously written as JSON to stdout. Disabled-path
  server startup verified by the full server + SDK integration suites (no
  regressions).

## Dependencies

- WP01 (needs the Collector + backends to validate against).
