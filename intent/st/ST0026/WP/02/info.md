---
verblock: "24 Jun 2026:v0.1: vscode - Initial version"
wp_id: WP-02
title: "Telemetry foundation and server enablement"
scope: Small
status: Not Started
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

- [ ] With `observability.enabled: false` (or unset), the server behaves exactly
  as today (JSON stdout logs only; no OTLP; no Collector dependency).
- [ ] With the WP01 stack up and `observability` configured, server traces appear
  in Tempo, server metrics in Prometheus, and OTLP logs in Loki - with JSON logs
  still on stdout.
- [ ] Invalid config (bad ratio, unreadable CA, malformed endpoint) is rejected
  by `validate()` with a clear typed error (No Silent Errors).
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test` pass.

## Dependencies

- WP01 (needs the Collector + backends to validate against).
