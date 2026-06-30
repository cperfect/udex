---
verblock: "30 Jun 2026:v0.1: vscode - Initial version"
wp_id: WP-04
title: "Test migration to ClickHouse, always-run obs"
scope: Small
status: Not Started
---

# WP-04: Test migration to ClickHouse, always-run obs

## Objective

Migrate the observability tests from Tempo/Prometheus/Loki HTTP queries to ClickHouse SQL, make them always-run (fail if obs is absent, like the Hydra tests), and add a non-k8s obs test path so coverage runs on every `cargo test`.

## Deliverables

- ClickHouse SQL query helpers (replacing `tempo_trace_span_names` / `prometheus_*` / `loki_*`), keyed off a run-specific span/entry attribute.
- Removal of `obs_stack_ready()` and the obs skip branches.
- A new non-k8s obs test: local `server -> collector -> ClickHouse` asserting traces/metrics/logs land, runnable in the main `cargo test` job.
- The existing `test_obs_k8s_*` tests requeried against ClickHouse.

## Acceptance Criteria

- [ ] Obs tests run unconditionally and FAIL (not skip) when ClickHouse is unreachable.
- [ ] The non-k8s obs test passes against the compose fixture without k3d.
- [ ] The k8s obs tests pass against ClickHouse.
- [ ] No remaining references to Tempo/Prometheus/Loki query endpoints in tests.

## Dependencies

- WP01 (data path), WP03 (postgres metrics + logs present for the metric/log assertions).
