# Implementation - ST0027: ClickHouse observability as an always-on compose fixture

## Implementation

### WP01 — ClickHouse + collector fixture (as-built)

- Added `clickhouse` (`clickhouse/clickhouse-server:24.8`) and `otel-collector` (`otel/opentelemetry-collector-contrib:0.119.0`, the image already in the tree) to `projects/compose/docker-compose.yml` as always-on peers of postgres/hydra.
- New static collector config `projects/compose/otel-collector/config.yaml`: OTLP/TLS receiver (4317/4318) -> `clickhouse` exporter for traces/metrics/logs. Fully version-controlled, no OpAMP.
- OTLP cert generation moved to `projects/compose/otel-collector/certs/regenerate_certs.sh`; `scripts/gen-keys-and-certs.sh` repointed (`OTLP_TLS_DIR` + the regenerate call); `.gitignore` updated. SANs unchanged (`otel-collector`, localhost, host.docker.internal, host.k3d.internal) since the service name is unchanged.
- Verified end-to-end from a cold start: OTLP trace over TLS (trusting `ca.crt`, via the `otel-collector` SAN) -> HTTP 200 -> queryable in `otel.otel_traces`.

### RESOLVED: schema decision (the WP01 de-risk)

The contrib `clickhouse` exporter creates the **exact** OTel schema HyperDX reads — `otel_traces`, `otel_logs`, `otel_metrics_{gauge,sum,histogram,exponential_histogram,summary}` (plus trace-id materialized views) — in the `otel` database. This matches what the spike observed inside the HyperDX all-in-one's ClickHouse. **No schema reconciliation is needed**; WP02 simply points HyperDX's source at the `otel` database. This was the primary risk for the whole steel thread and it is closed.

### WP02 — HyperDX + Mongo dev UI (as-built) + the bind-mount pivot

A devcontainer-rebuild bug surfaced while wiring HyperDX: the base compose is consumed with **two different project directories** (standalone/CI = `projects/compose/`, devcontainer = `.devcontainer/` since it is the first `-f` file and the overlay's own relative paths forbid reordering). Proof: the running postgres mounts `.devcontainer/postgres/...` (an empty stub), not `projects/compose/postgres/`. So every relative bind-mount path (the WP01 collector config + certs, and the HyperDX `env_file`) resolves to a non-existent `.devcontainer/...` location on a real devcontainer bring-up. Per the user decision (Path fix → "relax TLS, go bind-mount-free") the whole fixture was reworked to use **no bind mounts**:

- **otel-collector:** config inlined via Compose top-level `configs:` (`content:`), receiver is now **plaintext** (no TLS block, no certs). WP01's cert machinery was reverted: deleted `projects/compose/otel-collector/`, removed the OTLP wiring from `scripts/gen-keys-and-certs.sh`, reverted the `.gitignore` cert entries.
- **clickhouse:** unchanged (env-only, `CLICKHOUSE_DB=otel`).
- **mongo** (`mongo:5.0.32-focal`) + **hyperdx** (`docker.hyperdx.io/hyperdx/hyperdx:2`): HyperDX configured by **inline env** (no `env_file`). `DEFAULT_CONNECTIONS`/`DEFAULT_SOURCES` (format lifted from the ClickStack all-in-one entrypoint, retargeted to our `otel` db and the `clickhouse` service; session/rollup tables our stock collector does not create were dropped) pre-provision the connection + Logs/Traces/Metrics sources. `IS_LOCAL_APP_MODE` requests local single-user mode. Nothing `depends_on` HyperDX/Mongo, so they never gate readiness.

Verified cold, **with no path override**, in-context: collector ready, **plaintext** OTLP `POST -> 200 -> otel.otel_traces`; HyperDX boots, connects to Mongo, and on team creation seeds exactly: connection "Local ClickHouse" -> `http://clickhouse:8123`, and sources Logs/Traces/Metrics all on db `otel`.

Open UX note: `DEFAULT_*` seed only when the first team is created. A browser visit in local mode is expected to auto-bootstrap; headless, a one-time registration (POST `/api/register/password`, password >= 12 chars) creates the team and triggers the seed. Mongo is kept **ephemeral** (no volume) so the sources always re-seed fresh from the (git) env on each boot rather than drifting — the cost is re-bootstrapping the local user per fresh stack, which only affects the dev UI (CI does not use it). Confirm the in-browser auto-login behaviour and revisit if a volume is warranted.

## Technical Details

- **Database pre-creation:** the exporter's `create_schema` makes the tables but **not** the database, and it connects with `otel` as the session default — so `otel` must exist first. Created via `CLICKHOUSE_DB=otel` (env, no bind mount) rather than an `initdb.d` SQL file, because bind mounts misresolve under docker-outside-of-docker (see Challenges). `restart: unless-stopped` + the exporter's `retry_on_failure` self-heal any brief startup race.
- **TLS posture:** app -> collector is TLS (collector terminates with the generated cert; app trusts `ca.crt`). collector -> ClickHouse is in-network plaintext (`tcp://clickhouse:9000`), consistent with ST0026's internal hops.

## Challenges & Solutions

- **docker-outside-of-docker bind mounts.** Running `docker compose up` from *inside* the devcontainer passes devcontainer paths the host daemon cannot resolve, so a bind-mounted *file* (the collector config) is created as an empty *directory* at the target. This affects only ad-hoc manual bring-up from inside the devcontainer; the real lifecycle (VS Code devcontainer-create and CI runners) resolves relative paths correctly, exactly as the existing postgres mounts do. For verification we used a scratch override pointing the mounts at the host-absolute workspace path. Implication for WP04/WP06: tests/CI assume the fixture is brought up by the devcontainer/CI lifecycle (like hydra), not by an in-devcontainer `docker compose up`.
- **DB-creation crash loop.** Before `otel` existed the collector exited with `code 81 UNKNOWN_DATABASE` and crash-looped; resolved by `CLICKHOUSE_DB=otel` (above).
