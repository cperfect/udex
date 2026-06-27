---
verblock: "24 Jun 2026:v0.1: vscode - Initial version"
wp_id: WP-03
title: "Deep instrumentation datastore and server"
scope: Small
status: Done
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

- [x] A single create/lookup request produces a connected trace: server request
  span -> datastore query span(s), visible in Tempo.
- [x] Per-method request/error/latency metrics are queryable in Prometheus.
- [x] PostgreSQL receiver metrics (e.g. connections, commits, db size) are present
  in Prometheus.
- [x] An incoming `traceparent` is honoured (client trace id continues on the
  server span).
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test` pass.

## As-built notes

- **Cross-cutting via middleware, handlers untouched (Thin Coordinator).** Rather
  than per-handler `#[instrument]`, the request span + metrics live in middleware
  layered in `server::serve_with_listener`:
  - `tower_http::TraceLayer::new_for_grpc().make_span_with(...)` builds one span
    per request named by the gRPC method (`/pkg.Service/Method`), parented on the
    inbound W3C `traceparent`. Handler and datastore spans nest beneath it.
  - A small `udex_server::telemetry::MetricsLayer` (tower) records a per-method
    counter + latency histogram. gRPC status is read from the response (unary
    errors are trailers-only, so the `grpc-status` header carries the code;
    success => absent => 0). "Error count" is the counter filtered by
    `rpc.grpc.status_code != 0`.
- **Open-standard boundary preserved.** All `opentelemetry`/`tracing-opentelemetry`
  usage stays in `udex-telemetry`, which now exposes `make_request_span(method,
  headers)` (span + remote-parent attach, sets the global `TraceContextPropagator`)
  and `record_request(method, grpc_status, elapsed)`. The server middleware
  depends only on `tracing` + `http` + `tower`.
- **Datastore query spans** are `#[tracing::instrument(name = "db.<op>", skip_all,
  fields(...))]` on the 12 `Datastore` impl methods (create/lookup/get/delete
  entry + index ops + bulk read/write). Names like `db.create_entry`; fields
  carry index/key/op-count. No third-party error leakage.
- **Verified live** (stack up): a request span built from a crafted `traceparent`
  trace-id, with real datastore reads inside it, produced a single Tempo trace -
  retrievable by that exact trace-id - containing `/udex.smoke.v1.Smoke/Run`,
  `db.list_indices`, and `db.get_entry_by_key` (connected trace + traceparent
  honoured). `udex_rpc_requests_total` appeared in Prometheus; PostgreSQL receiver
  metrics confirmed in WP01. Full server + SDK + datastore suites pass through the
  new middleware with no regressions.

## Dependencies

- WP02 (telemetry foundation + server enablement).
