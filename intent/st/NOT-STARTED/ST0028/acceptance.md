---
verblock: "08 Aug 2026:v0.1: vscode - Initial version"
st_id: ST0028
title: "OpenObserve as the dev observability backend -- acceptance contract"
---

# ST0028 OpenObserve as the dev observability backend -- Acceptance

> Canonical acceptance contract for ST0028. Acceptance Criteria (AC) are the ratified completeness boundary; Acceptance Tests (AT) are the small red-to-green tests that prove them. Real test code lives in the suite (paths cited below); this file is the contract plus the AC-to-AT coverage map plus live status. info.md / WP info.md reference this file and never restate ACs (one home).
>
> Done = every AC is covered by a GREEN AT, or (for a non-test AC) its named evidence is satisfied, AND the AC set is the ratified full boundary. Done is read from this map, never from a hand-ticked box.
>
> Change control: clarifying an AC or AT is verifier-and-builder; shrinking scope, or weakening an AT to make it pass, needs the owner.
>
> AT status vocabulary: to-write (red-first) | red | green | n/a (non-test: doc / eyeball / gate).
>
> Non-test ACs carry their state inline -- `-- evidence: <ref> -- satisfied: yes|no` on the AC line; test-backed ACs are satisfied by a green covering AT (computed, never written). Multi-AC coverage on an AT is comma-separated.
>
> Exemption (ST0048): the close-gate is fail-by-default -- a unit with an empty or missing contract is refused. A unit that is deliberately AC-free (eg a pure content / authorial task) declares `acceptance: exempt` in the frontmatter above; the gate then passes and announces the exemption. Omit it (the default) and the contract is enforced. Never inferred from emptiness; always declared.

## Acceptance Criteria

### ST-level

- AC-00.1 No production crate changes: `udex-telemetry`, `udex-server`, `udex-sdk` and `udex-cli` source is untouched by this thread; the backend swap is visible only in compose, tests, CI, scripts and docs -- evidence: `git diff main --stat` limited to test files under `projects/rust/` -- satisfied: no
- AC-00.2 The observability fixture is three services (`openobserve`, `otel-collector`, `vector`), down from five -- evidence: `projects/compose/docker-compose.yml` -- satisfied: no
- AC-00.3 The fixture remains always-on and fail-never-skip: observability tests fail, never skip, when the backend is unreachable, exactly as the Hydra-dependent tests do -- evidence: helper source in `sdk/tests/common/mod.rs` -- satisfied: no

### WP-01 -- Stand up OpenObserve beside ClickHouse (status: WIP)

- AC-01.1 With the fixture up, `udex-server` traces, metrics and logs are all queryable in OpenObserve via its search API
- AC-01.2 The postgres/hydra container log floor reaches OpenObserve through the collector, in the same logs stream as application telemetry
- AC-01.3 The existing ClickHouse-backed observability tests still pass unchanged during this WP (the dual-export invariant that keeps the tree green)
- AC-01.4 (non-test) OpenObserve is configured with local-disk storage, telemetry reporting disabled, and retention equivalent to the current 72h -- evidence: `docker-compose.yml` service definition, confirmed live via `GET /config` returning `data_retention_days: 3`, `telemetry_enabled: false` -- satisfied: yes
- AC-01.5 (non-test) `gen-env.sh` generates the root credential and the pre-encoded basic-auth value; no credential is hardcoded in compose -- evidence: `scripts/gen-env.sh` run in an isolated tree; base64 round-trips to `email:password` and the password carries all four required character classes -- satisfied: yes
- AC-01.6 (non-test) All configuration is inline; no bind mounts are introduced, so the fixture resolves identically from `projects/compose/` and `.devcontainer/` -- evidence: `docker compose config -q` clean from both project directories -- satisfied: yes

### WP-02 -- Port the observability verification layer to OpenObserve (status: WIP -- AC-02.5 blocked, see AT-02.5)

- AC-02.1 A search helper that receives an OpenObserve API error fails loudly AND surfaces the API's own `message` (and `hint` when present), rather than reporting an empty result or a bare status code -- this is the `IN-AG-NO-SILENT-001` requirement and the thread's highest-risk detail
- AC-02.2 The trace assertion resolves an entry key to a trace and finds both the `/CreateEntry` request span and the `db.create_entry` datastore span, against OpenObserve
- AC-02.3 The metric assertion detects a run-scoped increase in `udex.rpc.requests` for `ListIndices`, against OpenObserve
- AC-02.4 The log assertion detects a run-scoped increase in `udex-server` log records, against OpenObserve
- AC-02.5 The three `test_obs_k8s_*` tests pass against OpenObserve with their assertions semantically unchanged
- AC-02.6 (non-test) No `clickhouse_*` helper or `otel.otel_*` table reference remains in the test suite -- evidence: `grep -ri clickhouse projects/rust/ --include=*.rs` returns only one deliberate comparative comment in `common/mod.rs` explaining the `"key"` quoting change -- satisfied: yes

### WP-03 -- New coverage: log floor and postgres receiver metric (status: Not Started)

- AC-03.1 An always-run test asserts postgres/hydra container logs reach the store with the correct `service_name`, alongside application telemetry -- coverage that did not exist before this thread
- AC-03.2 An always-run test asserts the collector's `postgresql.backends` receiver metric is present, closing the gap where it was only checked on the k8s path
- AC-03.3 (non-test) The floor test asserts on service name and body, not on severity, since severity is deliberately unset for floor records and its rendered value is an artifact -- evidence: test source and its comment -- satisfied: no

### WP-04 -- Retire ClickHouse, HyperDX and Mongo; CI and dev-doctor (status: Not Started)

- AC-04.1 The full test suite passes with `clickhouse`, `hyperdx`, `mongo` and `hyperdx-init` removed from the fixture
- AC-04.2 (non-test) CI starts only the surviving services and no job references `CLICKHOUSE_URL` -- evidence: `.github/workflows/01-Validation.yml` + a green pipeline run -- satisfied: no
- AC-04.3 (non-test) `dev-doctor.sh` checks OpenObserve reachability and version, with the exact-vs-major choice confirmed with the owner beforehand per project directive -- evidence: `scripts/dev-doctor.sh` + recorded decision -- satisfied: no
- AC-04.4 (non-test) No stale references to the removed services or their ports (`8123`, `8080`) remain anywhere in the repo -- evidence: repo-wide grep -- satisfied: no
- AC-04.5 (non-test) Actual CI fixture startup time recorded, so the "lighter than ClickHouse" assumption is measured rather than believed -- evidence: `impl.md` -- satisfied: no

### WP-05 -- Documentation (status: Not Started)

- AC-05.1 (non-test) No document describes ClickHouse or HyperDX as the observability backend -- evidence: repo-wide grep across `docs/`, `projects/*/README.md`, `.devcontainer/` -- satisfied: no
- AC-05.2 (non-test) The metrics-charting guidance is **replaced**, not deleted: a developer can still learn that the counters are cumulative and need a rate aggregation -- evidence: `projects/compose/README.md` -- satisfied: no
- AC-05.3 (non-test) `docs/SECRETS.md` documents the new credentials and states the posture change from keyless ClickHouse to an authenticated backend on a plaintext dev network -- evidence: `docs/SECRETS.md` -- satisfied: no
- AC-05.4 (non-test) ST0027's decision record is superseded rather than silently rewritten, so the history of why ClickHouse was chosen and then replaced survives -- evidence: `docs/DESIGN_DECISIONS.md` -- satisfied: no
- AC-05.5 (non-test) `/in-review` doc-reviewer pass run and its findings addressed -- evidence: review output -- satisfied: no

## Acceptance Tests

### WP-01

- AT-01.1 manual: query all three signals via the OpenObserve search API with the fixture up -- covers AC-01.1 -- status: green
- AT-01.2 manual: query the logs stream for `service_name IN ('postgres','hydra')` -- covers AC-01.2 -- status: green
- AT-01.3 `sdk/tests/obs.rs::obs_local_traces_metrics_logs_land` (unchanged, still reading ClickHouse) -- covers AC-01.3 -- status: green
- Coverage: AC-01.4, AC-01.5, AC-01.6 are non-test and carry evidence on the AC line. AT-01.1 and AT-01.2 become permanent tests in WP03; here they are one-shot confirmations that the pipeline is live.

AT-01.1 observed: traces returned all eight expected `udex-server` span names including `db.create_entry` and `/udex.entry.v1.EntryService/CreateEntry`; metrics returned `udex_rpc_requests` plus 19 `postgresql_*` receiver streams, with the cumulative counter query returning 3; logs returned `udex-server` records. AT-01.2 observed: `postgres` and `hydra` records in the same logs stream as `udex-server`. AT-01.3 observed: `test result: ok. 1 passed` -- the ClickHouse-backed assertions were unaffected by the dual-export.

### WP-02

- AT-02.1 `sdk/tests/obs.rs::obs_search_surfaces_query_errors` -- a deliberately malformed query must panic carrying the API's own reason, not return empty -- covers AC-02.1 -- status: green
- AT-02.2 `sdk/tests/obs.rs::obs_local_traces_metrics_logs_land` (trace assertion) -- covers AC-02.2 -- status: green
- AT-02.3 `sdk/tests/obs.rs::obs_local_traces_metrics_logs_land` (metric assertion) -- covers AC-02.3 -- status: green
- AT-02.4 `sdk/tests/obs.rs::obs_local_traces_metrics_logs_land` (log assertion) -- covers AC-02.4 -- status: green
- AT-02.5 `integration_tests.rs::test_obs_k8s_traces_land`, `::test_obs_k8s_metrics_land`, `::test_obs_k8s_logs_land` -- covers AC-02.5 -- status: **red (blocked, not run)**
- Coverage: AC-02.6 is non-test and carries evidence on the AC line. AC-02.5 is NOT satisfied -- see below.

AT-02.5 is blocked by a pre-existing, unrelated fault: the k3d `udex` deployment has been in `CrashLoopBackOff` for 34 days, failing DNS resolution of its database host (`failed to lookup address information`). With `K8S_SERVER_URL` unset the three tests take their early-return skip path and report `ok` in 0.00s -- which is a **skip, not a pass**, and must not be read as coverage.

The three tests are ported and compile, and their queries follow naming rules verified against real telemetry. One element remains genuinely unverified: the metric query filters on `deployment_environment = 'k3d'`. That column name follows the confirmed rule that resource attributes are flattened bare in the metrics stream (as `udex_test_run` was proven to be), but the specific attribute has never been observed, because no k8s telemetry has flowed for 34 days. It must be confirmed against a live cluster before AC-02.5 can go green.

Repairing the cluster is out of scope for this work package. AC-02.5 stays unsatisfied until a working deployment runs these tests.

### WP-03

- AT-03.1 `sdk/tests/obs.rs` (new): hydra container log reaches the store with `service_name = 'hydra'` -- covers AC-03.1 -- status: to-write (red-first)
- AT-03.2 `sdk/tests/obs.rs` (new): `postgresql.backends` present on the always-run path -- covers AC-03.2 -- status: to-write (red-first)
- Coverage: AC-03.3 is non-test and carries evidence on the AC line.

### WP-04

- AT-04.1 full `cargo test` run against the reduced three-service fixture -- covers AC-04.1 -- status: to-write (red-first)
- Coverage: AC-04.2 through AC-04.5 are non-test and carry evidence on their AC lines.

### WP-05

- Coverage: WP-05 is entirely non-test (documentation). Every AC carries named evidence on its own line; none is exempt.
