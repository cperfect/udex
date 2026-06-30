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

Auth/bootstrap (resolved): the modular `hyperdx:2` image has **no no-auth mode** — `IS_LOCAL_APP_MODE` exists only in the all-in-one's wrapper and is a no-op here (confirmed: the literal appears nowhere in the image), and there is no auth-disable env. So a login is required. A one-shot **`hyperdx-init`** service auto-registers the local user (`admin@udex.local` / `UdexLocalDev1!`) so a dev never meets a signup form — just log in. The seed is tied to first team creation, so this registration also triggers it. Hardening that mattered:
- HyperDX boot is slow/variable, so `hyperdx-init` is gated on a **healthcheck** (`wget .../health`) via `depends_on: condition: service_healthy` — no boot race.
- The register's server-side work (team + 3 sources) can outlast curl's read, returning an empty code despite succeeding; the init therefore confirms success by polling `/api/installation` for `isTeamExisting`, not the POST return code. Idempotent (skips if a team already exists).
- HyperDX password policy is >= 12 chars with upper + lower + digit + **special** (all four required).

Mongo is kept **ephemeral** (no volume) so the sources always re-seed fresh from the (git) env each boot rather than drifting; the cost is re-running the (automatic) bootstrap per fresh stack, which only affects the dev UI (CI does not use it). Browser render of the seeded sources still to be eyeballed by the user.

### WP03 — Postgres metrics + container-log floor (as-built)

- **Postgres metrics:** added the `postgresql` receiver to the inline collector config and wired it into the `metrics` pipeline -> `clickhouse`. Password read via the OTel env provider `$${env:POSTGRES_PASSWORD_SECRET}` (the `$$` escapes `$` past compose interpolation; the collector service gets `POSTGRES_PASSWORD_SECRET` from the same `.env` as the postgres service). Verified: 17 `postgresql.*` metrics land in `otel.otel_metrics_*`.
- **Container-log floor:** done with a slim **Vector** service, not the collector. The OTel `filelog` receiver was tried first and rejected: docker `json-file` logs carry no container identity (only `log`/`stream`/`time`; the name lives in the sibling `config.v2.json`), `add_metadata_from_filepath` only understands k8s paths, the files are root-only (needs `user: 0:0`), and a file reader slurps the *whole daemon* (all projects + k3d), unnamed. This is generic to Docker's json-file driver, not OrbStack. Container *names* require the Docker API, which Vector/Fluent Bit speak.
- Vector is **bind-mount-free in spirit**: config inlined via Compose `configs.content`; the only mount is the **absolute** `/var/run/docker.sock` (project-dir-independent). It tails via the Docker API, **filters by `com.docker.compose.service` to just `postgres`/`hydra`** (no obs-stack/k3d/other-project noise), and writes into `otel.otel_logs` with `ServiceName` = the compose service, so the floor shows up in HyperDX's Logs view next to app telemetry. Verified: postgres + hydra logs land named and scoped; nothing else.
- Severity note: postgres and hydra both log to **stderr** regardless of level, so stream != severity. `SeverityText` is left unset (the level is in the body) rather than mislabeling everything; the stream is kept as the `log.io.stream` log attribute.
- Net: `filelog` removed, collector back to its non-root default user. The user chose Vector over dropping the floor because postgres/hydra logs in ClickHouse were wanted.

### WP04 — Tests to ClickHouse, always-run obs (as-built)

- **Shared scaffolding extracted** to `projects/rust/sdk/tests/common/mod.rs` (user chose extract-over-duplicate): `server_cert_path`, `jwt_key_path`, `wait_for_server`, `make_token`, `context_input`, `now_unix_nanos` (moved out of `integration_tests.rs` — one copy), plus new **ClickHouse SQL helpers** (`clickhouse_query`/`clickhouse_trace_span_names`/`clickhouse_count`/`clickhouse_scalar_f64`) that **fail loudly** (never skip) if ClickHouse is unreachable — the hydra-style contract.
- **`integration_tests.rs`**: deleted all Tempo/Prometheus/Loki machinery (`obs_url`, `obs_stack_ready`, `tempo_*`, `prometheus_*`, `loki_count_since`, the `*_URL` envs) and the `obs_stack_ready()` skips; migrated the 3 `test_obs_k8s_*` tests to ClickHouse SQL. They still gate on `data_k8s()` (k8s fixture) but no longer skip on obs readiness.
- **New `tests/obs.rs`** — the always-run non-k8s path. Its own test binary because `udex_telemetry::init` installs a **process-global** subscriber and `integration_tests.rs` already installs a test subscriber (enabled-path `try_init` would collide). Stands up an in-process server with `observability` enabled, drives traffic, asserts trace+metric+log land in ClickHouse. **Verified passing (~11.5s).**
- **Key detail: OTLP is gRPC -> port 4317**, not HTTP 4318. The earlier manual probes used 4318 because curl spoke OTLP/HTTP, but the app's exporter is tonic/gRPC. `obs.rs` and the k8s config both target `4317`. The collector serves both ports, so no collector change was needed.
- **Metric query handles cumulative counters**: `udex.rpc.requests` is cumulative, so each export writes a new running-total row. The query takes `argMax(Value, TimeUnix)` per series (`service.instance.id` x `rpc.grpc.status_code`) and sums — monotonic across replicas. `obs.rs` scopes its baseline to `deployment.environment != 'k3d'` so concurrent k8s telemetry in the shared ClickHouse can't satisfy the increase; the k8s test scopes to `= 'k3d'`.
- **k8s flipped to plaintext** (overlaps WP06, needed for the k8s obs tests): `values.yaml` `otlpEndpoint -> http://host.k3d.internal:4317`; removed `otlp_ca` from the configmap, the OTLP-CA volume/mount from the deployment, and the `otlp-ca.crt` secret. `helm template` renders clean. The k8s obs tests were not *run* here (need a k3d redeploy with the new config) — to validate after WP06.
- Benign teardown noise on the obs run (`BatchLogProcessor`/meter-provider shutdown timeout as the guard drops at process exit) — cosmetic; the data lands and the test passes.

### WP05 — Agnostic `otlp_headers` config (as-built)

- Added `otlp_headers: BTreeMap<String, String>` to `TelemetryConfig` (serde default empty). Flows automatically into the server YAML (`observability:`) and the CLI config, since both embed the shared type — no separate plumbing.
- `lib.rs`: a `build_metadata()` builds a `tonic::MetadataMap` from the headers (via `http::HeaderName`/`HeaderValue` -> `MetadataMap::from_headers`) and is attached to all three OTLP exporters (`.with_metadata()` from `WithTonicConfig`). The headers apply to traces, metrics, and logs.
- **Secret hygiene:** header values are commonly API keys, so `TelemetryConfig` has a **manual `Debug`** that shows header names but redacts values (`<redacted>`); `validate()` rejects malformed names/values **without echoing the value**. (Nothing logs the config via Debug today, so this is defensive — satisfies the "header values not in logs" criterion regardless.)
- **CLI env:** `UDEX_OTLP_HEADERS` (comma-separated `key=value`, split on the first `=` so base64 survives) parsed by `parse_otlp_headers` in `init_cli_telemetry`.
- **Default behaviour unchanged:** empty headers -> empty `MetadataMap` -> identical to before. Our keyless fixture is unaffected.
- **Verification:** unit tests cover the mechanism (`build_metadata_carries_headers` proves the header reaches the exporter metadata), redaction (`debug_redacts_header_values`), validation (bad name/value rejected, value not leaked), and CLI parsing. A full live check against a header-requiring receiver is the documented manual path — the spike already proved the ClickStack all-in-one accepts a raw `authorization` key, and the README documents pointing Udex at it.
- **Docs:** `projects/rust/telemetry/README.md` gains a "Bringing your own OTLP backend" section (Honeycomb/Grafana-Cloud/ClickStack examples, incl. the all-in-one's raw-`authorization`/no-`Bearer` quirk); the `otlp_headers` field and `UDEX_OTLP_HEADERS` env are documented inline.

## Technical Details

- **Database pre-creation:** the exporter's `create_schema` makes the tables but **not** the database, and it connects with `otel` as the session default — so `otel` must exist first. Created via `CLICKHOUSE_DB=otel` (env, no bind mount) rather than an `initdb.d` SQL file, because bind mounts misresolve under docker-outside-of-docker (see Challenges). `restart: unless-stopped` + the exporter's `retry_on_failure` self-heal any brief startup race.
- **TLS posture:** app -> collector is TLS (collector terminates with the generated cert; app trusts `ca.crt`). collector -> ClickHouse is in-network plaintext (`tcp://clickhouse:9000`), consistent with ST0026's internal hops.

## Challenges & Solutions

- **docker-outside-of-docker bind mounts.** Running `docker compose up` from *inside* the devcontainer passes devcontainer paths the host daemon cannot resolve, so a bind-mounted *file* (the collector config) is created as an empty *directory* at the target. This affects only ad-hoc manual bring-up from inside the devcontainer; the real lifecycle (VS Code devcontainer-create and CI runners) resolves relative paths correctly, exactly as the existing postgres mounts do. For verification we used a scratch override pointing the mounts at the host-absolute workspace path. Implication for WP04/WP06: tests/CI assume the fixture is brought up by the devcontainer/CI lifecycle (like hydra), not by an in-devcontainer `docker compose up`.
- **DB-creation crash loop.** Before `otel` existed the collector exited with `code 81 UNKNOWN_DATABASE` and crash-looped; resolved by `CLICKHOUSE_DB=otel` (above).
