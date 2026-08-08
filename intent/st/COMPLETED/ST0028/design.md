---
verblock: "08 Aug 2026:v0.1: vscode - Initial version"
intent_version: 2.15.0
---

# Design - ST0028: OpenObserve as the dev observability backend

All findings cited here were verified on branch `spike/openobserve-obs` (`FINDINGS.md`) against real `udex-server` telemetry, not from documentation.

## Approach

Strangler swap in five work packages, each leaving the tree green.

```text
WP01  add OpenObserve; collector dual-exports; Vector -> collector      [green: old tests still read ClickHouse]
WP02  port the verification layer + existing obs tests to OpenObserve   [green: tests now read OpenObserve]
WP03  new coverage - Vector log floor, postgres receiver metric          [green: net-new assertions]
WP04  retire ClickHouse / HyperDX / Mongo; CI + dev-doctor              [green: only OpenObserve remains]
WP05  documentation                                                     [green]
```

The alternative — swapping backend and tests atomically in one change — was rejected because it puts the repo through a window where `cargo test` cannot pass, which trunk-based development should not require. Dual-export costs one extra list entry in the collector config and buys a green tree at every step.

## Design Decisions

### Storage: local disk, not object storage

`ZO_LOCAL_MODE=true` with `ZO_LOCAL_MODE_STORAGE=disk`; SQLite holds metadata. MinIO was the original proposal and was rejected during planning: it reintroduces a stateful service and a bucket bootstrap to a fixture whose purpose is to be light, and the object-storage path is not a Udex capability under test. The OTel capability that matters is the app's OTLP emission, which is identical either way.

No volume is declared, so telemetry is ephemeral and dies with the container. This matches the current fixture exactly (ClickHouse has no volume either) and is stated here so it reads as a choice rather than an oversight.

### The Collector stays, and stays the only backend-aware service

The app emits anonymous OTLP to a configurable endpoint and never learns the backend wants credentials. OpenObserve requires Basic auth on both ingest and query; that header is terminated in the collector's exporter config. This is the same separation ST0027 established for ClickStack's bearer token, and it is why no production behaviour changes in this thread.

```yaml
exporters:
  otlphttp/openobserve:
    endpoint: http://openobserve:5080/api/default
    headers:
      Authorization: "Basic ${env:OPENOBSERVE_BASIC_AUTH_SECRET}"
      stream-name: default
```

The `otlphttp` exporter appends `/v1/traces`, `/v1/metrics`, `/v1/logs`, which is exactly OpenObserve's ingest layout.

### Vector routes through the Collector

Vector keeps its `docker_logs` source — ST0027 established that the OTel `filelog` receiver cannot recover container *names*, which is the entire reason Vector exists — but its ClickHouse sink is replaced by an `opentelemetry` sink pointed at the collector. The collector then remains the single service that knows which backend exists (Highlander: one owner of "where telemetry goes").

The sink is thin and needs three non-obvious things, all verified in the spike: the OTLP `resourceLogs` envelope hand-built in VRL, the fallible form `to_unix_timestamp!(...)`, an explicit `Content-Type: application/json` request header (without it the collector answers `415` and Vector drops the batch as non-retriable), and `batch.max_events: 1` so batching cannot concatenate several JSON documents into one body.

One request per log line is acceptable for two low-volume dev containers and must carry a comment saying so. If it proves noisy, the documented fallback is the collector's `filelog` receiver plus a Docker metadata processor — accepting that it cannot name containers.

### Credentials come from `gen-env.sh`

The root credential is generated into `.env` alongside `POSTGRES_PASSWORD_SECRET`, not hardcoded like HyperDX's current `admin@udex.local`. OpenObserve rejects a trivial root password on first boot, so the generator must include punctuation.

Compose cannot base64-encode inline, so `gen-env.sh` emits **three** values: `OPENOBSERVE_ROOT_EMAIL`, `OPENOBSERVE_ROOT_PASSWORD_SECRET`, and the pre-encoded `OPENOBSERVE_BASIC_AUTH_SECRET` that the collector header and the tests both consume. Deriving it once at generation keeps the collector config declarative; the alternative (encoding at runtime) would need a shell wrapper in the collector container, which the fixture's no-bind-mount rule makes awkward.

Note the posture change this represents: ClickHouse today is keyless, so the fixture gains a credential travelling in plaintext over the compose network. It stays dev-only and loopback-published, which is consistent with ST0027, but `docs/SECRETS.md` must say so rather than let it pass unremarked.

### The search helper must surface errors

A rejected query comes back as **HTTP 400** with `{"code": ..., "message": "unknown field '...'", "hint": "..."}` — the very case the resource-attribute prefix asymmetry below invites. Some conditions instead answer 200 with `message` set and `hits` null, so both have to be handled.

The naive failure is not what the spike write-up first claimed. `error_for_status()` does catch the 400 — but it throws away `message` and `hint`, leaving a bare "400 Bad Request" to debug, when the response already named the offending column and suggested a repair. The helper therefore reads the body *before* reacting to the status and surfaces both.

(Corrected during WP02. The spike recorded this as "HTTP 200 with the error in the body"; it had only inspected the body via `jq` and never checked the status code. The requirement was right, the mechanism was not.)

`IN-AG-NO-SILENT-001` makes this a correctness requirement, not a nicety: the helper panics on a non-null `message`. This is an explicit AC because it is the single most likely source of a confusing failure in this thread.

### Metrics: `max(value)`, with its precondition written down

`max_by` does not exist in OpenObserve's DataFusion build, so `argMax(Value, TimeUnix)` has no direct port. Two forms were verified working; the chosen one is:

```sql
SELECT sum(m) AS total FROM (
  SELECT max(value) AS m FROM "udex_rpc_requests"
  WHERE rpc_method = '...' AND udex_test_run = '...'
  GROUP BY service_instance_id, rpc_grpc_status_code)
```

This is correct **only because** `udex.rpc.requests` is a monotonic cumulative counter, so the latest value per series is also its maximum. That sentence belongs in a comment beside the query. The faithful-but-noisier `first_value(...) OVER (PARTITION BY ... ORDER BY _timestamp DESC)` window form also works and is the fallback if the monotonic assumption ever stops holding.

## Architecture

```text
                         udex-server / udex-cli / k3d pods
                                      |  OTLP (unchanged, anonymous)
                                      v
   postgres+hydra ---> vector ---> otel-collector ---> OpenObserve  <--- integration tests
    (container         (docker      (+ postgresql       (store +          (SQL over HTTP,
     stdout)            API)         receiver)           query + UI)       Basic auth)
```

Everything to the left of the collector is unchanged in intent from ST0027; everything to the right collapses from four services to one.

### Schema mapping the verification layer encodes

| Signal  | ClickHouse today            | OpenObserve                                 |
| ------- | --------------------------- | ------------------------------------------- |
| traces  | `otel.otel_traces`          | stream `default`, `type=traces`             |
| traces  | `SpanName`                  | `operation_name`                            |
| traces  | `SpanAttributes['key']`     | `"key"` (quoted -- reserved-ish)            |
| traces  | `ParentSpanId`              | `reference_parent_span_id`                  |
| logs    | `otel.otel_logs`            | stream `default`, `type=logs`               |
| logs    | `Body` / `SeverityText`     | `body` / `severity`                         |
| metrics | `otel.otel_metrics_sum`     | one stream **per metric name**              |
| metrics | `MetricName`                | the stream name itself (`udex_rpc_requests`)|
| metrics | `Value` / `TimeUnix`        | `value` / `_timestamp`                      |
| all     | `Attributes['x']`           | bare flattened column (`rpc_method`)        |

### The prefix asymmetry

Resource attributes gain a `service_` prefix in the traces stream only:

| Signal  | `udex.test.run` becomes | `service.instance.id` becomes |
| ------- | ----------------------- | ----------------------------- |
| traces  | `service_udex_test_run` | `service_service_instance_id` |
| logs    | `udex_test_run`         | `service_instance_id`         |
| metrics | `udex_test_run`         | `service_instance_id`         |

This is the highest-risk detail in the thread. It is why the error-surfacing AC exists — with it, a wrong column name fails loudly and obviously; without it, it fails as a 90-second poll timeout blaming the pipeline.

Query API shape, for reference: `POST /api/{org}/_search?type={logs|traces|metrics}` with `{"query": {"sql", "start_time", "end_time", "from", "size"}}`. Times are **microseconds** and mandatory; a window that does not bracket the data returns zero hits with no error. The `type` parameter selects the signal, and the stream name `default` exists independently under both `logs` and `traces`, so omitting it silently searches the wrong signal.

## Test strategy

Coverage that exists today and is **ported, not rewritten** (WP02) — the ask to "add tests that traces, metrics and logs land" is already satisfied by these:

| Test                                            | Asserts                                                          |
| ----------------------------------------------- | ---------------------------------------------------------------- |
| `sdk/tests/obs.rs::obs_local_traces_metrics_logs_land` | all three signals, always-run, no cluster needed            |
| `integration_tests.rs::test_obs_k8s_traces_land`  | trace spans from the k3d deployment                             |
| `integration_tests.rs::test_obs_k8s_metrics_land` | `udex.rpc.requests` rises; `postgresql.backends` present        |
| `integration_tests.rs::test_obs_k8s_logs_land`    | `udex-server` log count rises                                   |

Genuinely **net-new** coverage (WP03), filling gaps ST0027 left:

- **Vector log floor.** ST0027 built postgres/hydra container-log shipping and nothing asserts it arrives. Under `IN-AG-NO-SILENT-001` and the project's "if it isn't tested it doesn't work" rule this is a real hole, and it widens in this thread because the floor's transport changes from a direct sink to an OTLP hop.
- **`postgresql.backends` on the always-run path.** Currently asserted only in the k8s test, so a collector-receiver regression is invisible unless someone runs the cluster suite.

Fixture posture is inherited from ST0027 unchanged: always-on, and the helpers **fail rather than skip** when the backend is unreachable, exactly like the Hydra-dependent tests.

## Alternatives Considered

**MinIO-backed object storage** — the original proposal. Rejected: adds back a stateful service and bucket bootstrap against a simplification goal, and exercises a path that is not a Udex capability. Revisit only if the dev fixture ever needs to mirror a production storage topology.

**Atomic backend + test swap** — rejected: requires a red window mid-thread, which trunk-based development should not need. Dual-export makes the strangler sequence nearly free.

**Keep HyperDX pointed at OpenObserve** — not viable; HyperDX reads ClickHouse specifically, and OpenObserve's bundled UI is the reason Mongo and `hyperdx-init` can be deleted at all.

**Vector writing to OpenObserve directly** — rejected: it duplicates backend knowledge across two configs (Highlander), for a smaller diff than routing through the collector.

**Collector `filelog` receiver instead of Vector** — ST0027 already established this cannot recover container names from docker json-file logs, which is why Vector exists. Retained only as the documented fallback if per-line OTLP requests prove too noisy.

## Open questions raised at planning — all resolved

These were the unknowns this thread deliberately refused to assume its way past. Each is settled; the original wording is kept so the question and its answer sit together.

- **Retention.** *Asked:* ClickHouse's exporter sets `ttl: 72h`; the equivalent `ZO_COMPACT_DATA_RETENTION_DAYS` is day-granular, so 72h maps to `3` — unverified in the spike. **Resolved in WP01:** confirmed against the running service rather than the config file. `GET /config` returns `data_retention_days: 3` and `telemetry_enabled: false` (AC-01.4).
- **CI footprint.** *Asked:* unmeasured; OpenObserve *should* be lighter than ClickHouse + Mongo + HyperDX, but CI is where the fixture has to be reliable, so WP04 should record actual startup time rather than assume an improvement. **Resolved in WP04:** the three observability services cold-start in **~4s**, and backend image footprint drops from ~2.73GB to 525MB. Stated as an absolute measurement, not a comparison — the old stack's startup was never timed before it was removed (AC-04.5).
- **UI ergonomics.** *Asked:* nobody had driven the OpenObserve UI to confirm the charting story was at least as good as the HyperDX recipes, and the developer UI is a main reason the fixture exists. **Resolved in WP05:** it is, and better in one respect ST0027 had recorded as a loss — OpenObserve answers **PromQL**, so cumulative counters take `rate()` natively where ClickHouse SQL needed explicit `argMax`-per-series. Every replacement recipe in `projects/compose/README.md` was executed against the running fixture, and each cited UI route (`/web/logs`, `/web/traces`, `/web/metrics`, `/web/dashboards`, `/web/streams`) was probed rather than assumed (AC-05.2).
