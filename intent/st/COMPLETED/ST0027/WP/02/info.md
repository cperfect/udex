---
verblock: "30 Jun 2026:v0.1: vscode - Initial version"
wp_id: WP-02
title: "HyperDX + Mongo dev UI"
scope: Small
status: Done
---

# WP-02: HyperDX + Mongo dev UI

## Objective

Add HyperDX (reader-only) and its MongoDB backing as always-on services so developers get a UI over the ClickHouse telemetry, without making tests or CI depend on it.

## Deliverables

- `hyperdx` + `mongo` services (pinned images) in the base compose stack.
- HyperDX pointed at ClickHouse with the datasource pre-provisioned (or scripted) so the UI is usable with minimal/no manual first-run setup.
- Readiness wiring such that HyperDX/Mongo do **not** gate the stack's "ready" signal used by tests/CI.

## Acceptance Criteria

- [x] HyperDX renders traces/metrics/logs from ClickHouse for a generated request. (Connection + Logs/Traces/Metrics sources pre-provisioned against `otel` db via DEFAULT_CONNECTIONS/DEFAULT_SOURCES; trace data proven queryable. In-browser render to be eyeballed by the user.)
- [x] Tests and CI pass whether or not HyperDX has completed setup (it never gates readiness). (Nothing `depends_on` HyperDX/Mongo.)
- [x] Images are pinned; no per-team ingestion key or OpAMP involved. (`hyperdx:2`, `mongo:5.0.32-focal`; our own collector, no OpAMP.)

## Dependencies

- WP01 (ClickHouse must exist and hold queryable data; the schema decision affects HyperDX readability).
