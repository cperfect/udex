---
verblock: "24 Jun 2026:v0.1: vscode - Initial version"
wp_id: WP-06
title: "Integration tests idempotent"
scope: Small
status: Not Started
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

- [ ] Tests assert traces, metrics (app + postgres), and logs all land for a
  tagged operation.
- [ ] Tests are idempotent - uniquely tagged per run, reruns do not collide, no
  shared mutable state assumptions.
- [ ] Tests are reliable (bounded polling with clear failure messages; no flakey
  sleeps-and-hope).
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test` pass with the stack up.

## Dependencies

- WP02-WP05 (the instrumentation and deployments under test).
