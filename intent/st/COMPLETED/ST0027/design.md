# Design - ST0027: ClickHouse observability as an always-on compose fixture

## Approach

Replace the standalone six-service `projects/observability/` stack with a single ClickHouse-backed OpenTelemetry pipeline folded into `projects/compose/` and started by default with the base stack (peers of `postgres`/`hydra`). Keep our own statically-configured stock collector so config lives in git; use ClickHouse as the unified store; ride HyperDX on top as a reader-only UI. Make observability a hard test dependency (always run, fail if absent) and keep the application solution-agnostic (pure OTLP out).

Always-on services added to `projects/compose/docker-compose.yml`:

| Service | Image | Role | Notes |
|---|---|---|---|
| `otel-collector` | `otel/opentelemetry-collector-contrib` (already used) | OTLP(TLS) ingest + `postgresqlreceiver` + container-log `filelog`/docker receiver; exports to ClickHouse | Static config in git; no OpAMP |
| `clickhouse` | `clickhouse/clickhouse-server` | Unified store for traces/metrics/logs | Replaces Tempo + Prometheus + Loki |
| `hyperdx` | HyperDX OSS (reader-only) | Dev UI over ClickHouse | Always-on incl CI (per decision); tests do not depend on it |
| `mongo` | `mongo` | HyperDX application state | Required by HyperDX |

Data flow:

```text
udex server/sdk/cli --OTLP/TLS--> otel-collector --> clickhouse <-- HyperDX (UI, reads)
postgres (server metrics) --postgresqlreceiver--> otel-collector
container stdout (postgres/hydra/app) --filelog/docker receiver--> otel-collector
```

## Design Decisions

1. **Modular, not all-in-one.** The spike (findings in `FINDINGS.md` on the `spike/clickstack-observability` branch; not on this branch) showed the all-in-one is OpAMP/Mongo control-plane-driven, needs first-run setup, and gates OTLP behind a per-team bearer token — unfit for a deterministic git-tracked fixture. Owning the collector dissolves all three.
2. **Stock contrib collector, static config.** Reuse `otel/opentelemetry-collector-contrib` (the `clickhouse` exporter ships in contrib). No ClickStack collector image, no OpAMP, no runtime config mutation — satisfies "Configuration MUST NOT be mutated at runtime."
3. **Solution-agnostic boundary.** Nothing ClickStack-specific in `udex-server`/`udex-sdk`/`udex-cli`/`udex-telemetry`. Because we own the collector receiver (keyless + TLS), the ClickStack auth-header peculiarity never reaches the app — it keeps emitting plain OTLP over TLS with no `Authorization` header. The optional `otlp_headers` config (WP05) is a generic OTLP feature for users' own header-authed backends, not a ClickStack coupling.
4. **Bind-mount-free fixture, plaintext OTLP (revised during WP02).** The base compose is consumed with two different project directories — standalone/CI (`projects/compose/`) and the devcontainer (`.devcontainer/`, the first `-f` file; the overlay's own `../.env` / `context: .` paths forbid reordering). A relative bind-mount path therefore resolves to two different places and breaks in one. So the fixture uses **no bind mounts**: the collector config is supplied inline via Compose `configs.content`, HyperDX is configured by inline env, ClickHouse by env (`CLICKHOUSE_DB`). The corollary is the OTLP hop is **plaintext** — TLS would require mounting cert files (the same path trap), and the app already supports plaintext OTLP (`http://` endpoint, no CA; the `otlp_ca`-only-for-https rule from ST0026 round-3). This relaxes the ST0026 app->collector TLS for the *local dev/CI fixture only*; the app stays OTLP-standard and can target a TLS-terminating collector in any real deployment. collector -> ClickHouse stays in-network plaintext. (This supersedes the original "keep TLS / generate collector certs" plan; the cert machinery added in early WP01 was removed.)
5. **HyperDX always-on in local dev; omitted from CI (refined in WP06).** The UI is always-on for a developer (where a human uses it) but is **not started in CI**: it has no test value (tests query ClickHouse directly), and `hyperdx-init`'s `depends_on: service_healthy` would gate the CI step on HyperDX's slow boot plus a 1.8 GB image pull. CI starts only the test-relevant obs services (`clickhouse` + `otel-collector` + `vector`). (This refines the earlier "always-on everywhere incl CI" decision once the boot-gating cost was clear; user-confirmed.)
6. **Log floor via a slim Vector (revised during WP03).** The plan was to fold the floor into the collector via `filelog` and retire Vector — but docker `json-file` logs carry no container identity (only `log`/`stream`/`time`), `filelog` would slurp the whole daemon unnamed, and it needs a root collector. Container *names* require the Docker API. So the documented fallback was taken: a slim, bind-mount-free **Vector** (inline config + absolute `docker.sock`) tails via the API, filters by compose service to postgres/hydra, and writes to `otel.otel_logs` (`ServiceName` = service) so the floor appears in HyperDX. Generic Docker limitation, not OrbStack. Vector is **retained**, not retired.

## Architecture

What is removed: the entire `projects/observability/` directory (compose, `collector/`, `tempo/`, `prometheus/`, `loki/`, `vector/`, `grafana/`, `scripts/`, `certs/`, README, and the `spike-clickstack/` artifacts once folded here); the `OBS_HOST_BIND`/`OBS_HOST_DIR`/`OBS_NETWORK` machinery and separate-project port-forward gymnastics; the Vector service.

Testing strategy:

- **Query layer:** replace Tempo/Prometheus/Loki HTTP helpers with **ClickHouse SQL** helpers (HTTP `:8123` or client), asserting spans/metrics/logs land for a run-specific key.
- **Always-run:** delete `obs_stack_ready()`; obs tests run unconditionally and fail if ClickHouse is unreachable — same contract as the Hydra tests.
- **Two paths:** a new **non-k8s** `server -> collector -> ClickHouse` test in the main `cargo test` job (exercised every run, independent of the k8s path-filter), plus the migrated **k8s** `test_obs_k8s_*` tests requeried against ClickHouse.

CI (`01-Validation.yml`): the base compose `up` brings up the obs services in both the `test` and `k8s-test` jobs; obs tests run in both and must pass (reverses the prior "obs not in CI" stance). Update `scripts/dev-doctor.sh` (new images clickhouse + collector; drop the four replaced) and the version-pinning policy (ask exact-vs-major per the CLAUDE.md directive).

Risks:

- **ClickHouse <-> HyperDX schema mismatch (primary).** The contrib `clickhouse` exporter creates its own schema; HyperDX expects specific tables (`otel_traces`/`otel_logs`/`otel_metrics_*`). WP01 reconciles them — match the exporter's tables to HyperDX's, or accept HyperDX as best-effort and point *tests* at the exporter's tables. Test correctness targets ClickHouse directly, so it does not depend on HyperDX compatibility.
- **CI weight/flakiness** from ClickHouse + HyperDX + Mongo always-on — mitigate by not gating on HyperDX readiness and pinning images.
- **Container-log access under docker-outside-of-docker** for filelog — needs the right host mount; fall back to a slim Vector if impractical.

## Alternatives Considered

- **ClickStack all-in-one image.** Rejected: OpAMP/Mongo control plane, first-run setup, per-team bearer token, runtime-pushed collector config — conflicts with deterministic, git-tracked, no-runtime-mutation requirements (spike evidence).
- **Keep the ST0026 Grafana stack, just wire into compose.** Rejected: six services, four separate stores/retention stories, and the bespoke external-network attach dance; heavier than a single ClickHouse store and harder to make a clean always-on fixture.
- **App sends the ClickStack ingestion header directly.** Rejected: couples the application to a specific backend, violating the solution-agnostic goal. Owning the collector removes the need entirely.

## Work packages

- **WP01 — ClickHouse + collector fixture in compose.** Add `clickhouse` + reconfigured `otel-collector` (OTLP/TLS receiver + `clickhouse` exporter) as always-on services; wire generated OTLP certs through `gen-keys-and-certs.sh`. Prove app -> collector -> ClickHouse. Resolve the schema-vs-HyperDX question.
- **WP02 — HyperDX + Mongo UI.** Add `hyperdx` + `mongo`, point HyperDX at ClickHouse, pre-provision the datasource; ensure it does not gate readiness.
- **WP03 — postgresqlreceiver + log floor.** Fold Postgres server metrics and the container-stdout floor into the collector; retire Vector.
- **WP04 — Test migration.** ClickHouse SQL helpers; remove `obs_stack_ready()`; add the non-k8s obs test; requery the k8s obs tests; obs always runs.
- **WP05 — Agnostic `otlp_headers` config.** Optional OTLP header support in `udex-telemetry` (+ server/CLI surface) for header-authed third-party backends; document as the "plug your own backend" path.
- **WP06 — Decommission + docs + CI.** Delete `projects/observability/`; update `01-Validation.yml`, `dev-doctor.sh`, ARCHITECTURE/CONTRIBUTING/FAQ, and a new compose-level observability README.
