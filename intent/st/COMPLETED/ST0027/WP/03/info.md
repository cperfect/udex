---
verblock: "30 Jun 2026:v0.1: vscode - Initial version"
wp_id: WP-03
title: "postgresqlreceiver + container-log floor in collector"
scope: Small
status: Done
---

# WP-03: postgresqlreceiver + container-log floor in collector

## Objective

Fold the two ingestion paths the current stack handles outside plain OTLP into the collector config: PostgreSQL server metrics (`postgresqlreceiver`) and the container-stdout "durable log floor" (postgres/hydra/app), retiring the separate Vector service.

## Deliverables

- `postgresql` receiver in the collector config -> ClickHouse (mirrors ST0026's `postgresql_backends`-style metrics), password from `.env`.
- Container-stdout log floor -> ClickHouse `otel_logs`, named and scoped to postgres/hydra.
- `postgresql` wired into the `metrics` pipeline; the floor handled by a slim Vector service.

> **Decision:** the `filelog`-in-collector route was tried and rejected — docker `json-file` logs carry no container identity, so it can't name or scope logs (it would slurp the whole daemon, unnamed) and needs a root collector. This is generic to Docker, not OrbStack. The documented fallback was taken: a slim, **bind-mount-free** Vector (inline config + absolute `docker.sock`) that uses the Docker API to get names and filter by compose service. The user chose this over dropping the floor because postgres/hydra logs in ClickHouse were wanted.

## Acceptance Criteria

- [x] PostgreSQL server metrics are queryable in ClickHouse. (17 `postgresql.*` metrics in `otel.otel_metrics_*`.)
- [x] postgres/hydra container stdout lands in ClickHouse logs (the floor is preserved), scoped to only those services — obs stack / k3d / other-project containers excluded (no feedback loop). (App logs arrive via OTLP; there is no app *container* in the compose stack.)
- [x] Vector decision recorded; no functionality regressed vs ST0026. (Vector **retained** as the named/scoped floor — see Decision above; the collector is back to its non-root default.)

## Dependencies

- WP01 (collector + ClickHouse exist). Fallback: keep a slim Vector -> collector OTLP if the filelog receiver proves impractical under docker-outside-of-docker.
