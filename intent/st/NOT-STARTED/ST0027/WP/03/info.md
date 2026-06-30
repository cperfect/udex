---
verblock: "30 Jun 2026:v0.1: vscode - Initial version"
wp_id: WP-03
title: "postgresqlreceiver + container-log floor in collector"
scope: Small
status: Not Started
---

# WP-03: postgresqlreceiver + container-log floor in collector

## Objective

Fold the two ingestion paths the current stack handles outside plain OTLP into the collector config: PostgreSQL server metrics (`postgresqlreceiver`) and the container-stdout "durable log floor" (postgres/hydra/app), retiring the separate Vector service.

## Deliverables

- `postgresql` receiver in the collector config -> ClickHouse (mirrors ST0026's `postgresql_backends`-style metrics), password from `.env`.
- A container-log receiver (`filelog` or docker-logs) -> ClickHouse, with the docker-outside-of-docker host log-path mount solved; Vector removed.
- A `metrics/postgresql` (and `logs/...`) pipeline wired to the `clickhouse` exporter.

## Acceptance Criteria

- [ ] PostgreSQL server metrics are queryable in ClickHouse.
- [ ] postgres/hydra/app container stdout lands in ClickHouse logs (the floor is preserved), with the obs stack's own containers excluded (no feedback loop).
- [ ] The Vector service is gone; no functionality regressed vs ST0026.

## Dependencies

- WP01 (collector + ClickHouse exist). Fallback: keep a slim Vector -> collector OTLP if the filelog receiver proves impractical under docker-outside-of-docker.
