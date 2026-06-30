# Implementation - ST0027: ClickHouse observability as an always-on compose fixture

## Implementation

### WP01 — ClickHouse + collector fixture (as-built)

- Added `clickhouse` (`clickhouse/clickhouse-server:24.8`) and `otel-collector` (`otel/opentelemetry-collector-contrib:0.119.0`, the image already in the tree) to `projects/compose/docker-compose.yml` as always-on peers of postgres/hydra.
- New static collector config `projects/compose/otel-collector/config.yaml`: OTLP/TLS receiver (4317/4318) -> `clickhouse` exporter for traces/metrics/logs. Fully version-controlled, no OpAMP.
- OTLP cert generation moved to `projects/compose/otel-collector/certs/regenerate_certs.sh`; `scripts/gen-keys-and-certs.sh` repointed (`OTLP_TLS_DIR` + the regenerate call); `.gitignore` updated. SANs unchanged (`otel-collector`, localhost, host.docker.internal, host.k3d.internal) since the service name is unchanged.
- Verified end-to-end from a cold start: OTLP trace over TLS (trusting `ca.crt`, via the `otel-collector` SAN) -> HTTP 200 -> queryable in `otel.otel_traces`.

### RESOLVED: schema decision (the WP01 de-risk)

The contrib `clickhouse` exporter creates the **exact** OTel schema HyperDX reads — `otel_traces`, `otel_logs`, `otel_metrics_{gauge,sum,histogram,exponential_histogram,summary}` (plus trace-id materialized views) — in the `otel` database. This matches what the spike observed inside the HyperDX all-in-one's ClickHouse. **No schema reconciliation is needed**; WP02 simply points HyperDX's source at the `otel` database. This was the primary risk for the whole steel thread and it is closed.

## Technical Details

- **Database pre-creation:** the exporter's `create_schema` makes the tables but **not** the database, and it connects with `otel` as the session default — so `otel` must exist first. Created via `CLICKHOUSE_DB=otel` (env, no bind mount) rather than an `initdb.d` SQL file, because bind mounts misresolve under docker-outside-of-docker (see Challenges). `restart: unless-stopped` + the exporter's `retry_on_failure` self-heal any brief startup race.
- **TLS posture:** app -> collector is TLS (collector terminates with the generated cert; app trusts `ca.crt`). collector -> ClickHouse is in-network plaintext (`tcp://clickhouse:9000`), consistent with ST0026's internal hops.

## Challenges & Solutions

- **docker-outside-of-docker bind mounts.** Running `docker compose up` from *inside* the devcontainer passes devcontainer paths the host daemon cannot resolve, so a bind-mounted *file* (the collector config) is created as an empty *directory* at the target. This affects only ad-hoc manual bring-up from inside the devcontainer; the real lifecycle (VS Code devcontainer-create and CI runners) resolves relative paths correctly, exactly as the existing postgres mounts do. For verification we used a scratch override pointing the mounts at the host-absolute workspace path. Implication for WP04/WP06: tests/CI assume the fixture is brought up by the devcontainer/CI lifecycle (like hydra), not by an in-devcontainer `docker compose up`.
- **DB-creation crash loop.** Before `otel` existed the collector exited with `code 81 UNKNOWN_DATABASE` and crash-looped; resolved by `CLICKHOUSE_DB=otel` (above).
