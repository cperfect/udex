---
verblock: "24 Jun 2026:v0.1: vscode - Initial version"
wp_id: WP-06
title: "Integration tests idempotent"
scope: Small
status: Done
---

# WP-06: Integration tests (idempotent)

## Objective

Add idempotent integration tests that prove the observability features work by
querying component APIs / data sources for telemetry produced by a tagged
operation. Tests must be reliable and safe to rerun.

## Deliverables

- New `test_obs_` (local stack) and `test_obs_k8s_` (cluster) integration tests:
  - **Traces**: perform a uniquely-tagged operation, then query Tempo for a trace
    containing the tag (bounded polling).
  - **Metrics**: query Prometheus (instant query) for app metrics and for
    `postgresqlreceiver` metrics.
  - **Logs**: query Loki for a uniquely-tagged log line (both OTLP and stdout
    paths).
- Shared helpers (in `udex-test-utils` where appropriate) for backend queries and
  bounded-retry polling.
- Skip/guard behaviour consistent with existing suites (e.g. backends not up ->
  clearly skipped, mirroring the `K8S_SERVER_URL` pattern), without skipping when
  the stack is available.
- Naming convention prefixes added to the test suite.

## Acceptance Criteria

- [x] Tests assert traces, metrics (app + postgres), and logs all land for a
  tagged operation (`test_obs_k8s_traces_land`, `_metrics_land`, `_logs_land`).
- [x] Tests are idempotent - the trace test keys off a freshly-created entry's
  unique server key, metric/log tests are presence checks; verified by a clean
  rerun.
- [x] Tests are reliable (bounded retry polling with clear failure messages;
  clean skip when the stack is unreachable).
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test` pass with the stack up.

## As-built notes

- **Three `test_obs_k8s_*` tests** in `sdk/tests/integration_tests.rs`, reusing
  the existing `data_k8s` fixture (authenticated client against the
  observability-enabled k3d deployment from WP05):
  - `traces_land`: creates an entry with a unique context; the server stamps the
    generated key onto the `db.create_entry` span `key` attribute, so the test
    queries Tempo (`{ span.key = "<uuid>" }`) and asserts the trace contains the
    `CreateEntry` request span and `db.create_entry` (connected request->datastore).
  - `metrics_land`: asserts `udex_rpc_requests_total{deployment_environment="k3d"}`
    and `postgresql_backends` are present in Prometheus.
  - `logs_land`: asserts `{service_name="udex-server"}` log lines are present in Loki.
- **Tagging without host telemetry**: tests do not init OpenTelemetry in the test
  process (which would conflict with the shared global subscriber across the test
  binary). The k3d pods export their own telemetry (WP05); the unique entry key is
  the run-specific handle, avoiding any in-process telemetry setup. This is also
  why a separate in-process `test_obs_` (local server) variant was not added - it
  would fight the shared subscriber; the k8s tests already exercise the local
  Tempo/Prometheus/Loki backends end to end.
- **Helpers** (`obs_stack_ready`, `tempo_trace_span_names`,
  `prometheus_series_count`, `loki_line_count`) live in the test file rather than
  `udex-test-utils`: that crate has no HTTP stack (only hydra/secrets fixtures),
  and the test file is the sole consumer. Each helper bounded-polls (~60s) and the
  reachability guard skips cleanly when the stack is down. Backend URLs default to
  the devcontainer service names and are overridable via `TEMPO_URL` /
  `PROMETHEUS_URL` / `LOKI_URL`.
- **Verified live**: 3/3 pass against the cluster + local stack; a second run also
  passes (idempotent); with the backends pointed at a dead port the tests skip
  cleanly. Run them with `bash projects/observability/scripts/up.sh` then
  `K8S_SERVER_URL=... cargo test -p udex-sdk --test integration_tests test_obs_k8s`.

## Dependencies

- WP02-WP05 (the instrumentation and deployments under test).
