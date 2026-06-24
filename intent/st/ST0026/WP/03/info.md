---
verblock: "24 Jun 2026:v0.1: vscode - Initial version"
wp_id: WP-03
title: "Deep instrumentation datastore and server"
scope: Small
status: Not Started
---

# WP-03: Deep instrumentation datastore and server

## Objective

Add meaningful spans and metrics across the request hot path: datastore query
spans (sqlx), server handler spans + core gRPC metrics, server-side W3C context
extraction so client traces continue through the server, and validation of the
Collector `postgresqlreceiver` against the live database.

## Deliverables

- `udex-datastore`: sqlx query/transaction instrumentation producing spans
  (operation name, table/statement category, timing), correlated with the request
  trace. No third-party error leakage.
- `udex-server`: handler spans (`#[instrument]` or explicit) on entry/index
  operations with relevant attributes (index, operation, result), plus core
  metrics (per-method request count, error count, latency histogram) emitted via
  the `udex-telemetry` meter.
- Server-side W3C `traceparent` extraction from incoming gRPC metadata so a
  client-supplied trace context becomes the parent span.
- Collector `postgresqlreceiver` validated end to end: PostgreSQL server metrics
  visible in Prometheus.

## Acceptance Criteria

- [ ] A single create/lookup request produces a connected trace: server handler
  span -> datastore query span(s), visible in Tempo.
- [ ] Per-method request/error/latency metrics are queryable in Prometheus.
- [ ] PostgreSQL receiver metrics (e.g. connections, commits, db size) are present
  in Prometheus.
- [ ] An incoming `traceparent` is honoured (client trace id continues on the
  server span).
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test` pass.

## Dependencies

- WP02 (telemetry foundation + server enablement).
