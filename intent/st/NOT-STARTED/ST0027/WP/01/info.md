---
verblock: "30 Jun 2026:v0.1: vscode - Initial version"
wp_id: WP-01
title: "ClickHouse + collector fixture in compose"
scope: Medium
status: Not Started
---

# WP-01: ClickHouse + collector fixture in compose

## Objective

Stand up ClickHouse and a reconfigured stock `otel/opentelemetry-collector-contrib` as always-on services in `projects/compose/docker-compose.yml`, exporting OTLP traces/metrics/logs to ClickHouse, and prove the end-to-end data path. This WP de-risks the steel thread by resolving the primary unknown: the ClickHouse table schema the contrib exporter writes vs. what HyperDX expects to read.

## Deliverables

- `clickhouse` service (pinned image) added to the base compose stack.
- `otel-collector` service reconfigured: OTLP/TLS receiver (collector cert, app trusts the CA) -> `clickhouse` exporter for all three signals; static config in git.
- OTLP cert generation wired through `scripts/gen-keys-and-certs.sh` (moved out of `projects/observability/certs`).
- A documented decision on the schema question (match the exporter tables to HyperDX's, or treat HyperDX as best-effort and target tests at the exporter's tables).

## Acceptance Criteria

- [ ] `docker compose up` in `projects/compose` starts `clickhouse` + `otel-collector` alongside postgres/hydra, both reaching ready without manual steps.
- [ ] A real OTLP trace/metric/log from the udex app (TLS) lands and is queryable in ClickHouse via SQL.
- [ ] The collector config is fully static (no OpAMP, no runtime mutation) and committed.
- [ ] The schema-vs-HyperDX decision is recorded in the ST design/impl notes.

## Dependencies

- None (first WP; de-risks WP02 and WP04).
