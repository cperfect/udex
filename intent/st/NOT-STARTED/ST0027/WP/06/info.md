---
verblock: "30 Jun 2026:v0.1: vscode - Initial version"
wp_id: WP-06
title: "Decommission projects/observability + docs + CI"
scope: Small
status: Not Started
---

# WP-06: Decommission projects/observability + docs + CI

## Objective

Decommission the old stack and make the new fixture the documented default everywhere: delete `projects/observability/`, update CI to run obs always-on, and refresh tooling/docs.

## Deliverables

- `projects/observability/` removed (compose, collector/tempo/prometheus/loki/vector/grafana configs, scripts, certs, README, `spike-clickstack/`).
- `01-Validation.yml`: obs services come up as part of the base stack in the `test` and `k8s-test` jobs; obs tests run and must pass in both (reverses the prior "obs not in CI" stance).
- `scripts/dev-doctor.sh` updated (new images clickhouse + collector; drop tempo/loki/prometheus/grafana/vector), with the exact-vs-major version policy confirmed with the user.
- Docs refreshed: a compose-level observability README, plus `ARCHITECTURE.md` / `CONTRIBUTING.md` / `FAQ.md` updates; ST0026 references reconciled.

## Acceptance Criteria

- [ ] No references to `projects/observability/` remain in code, scripts, CI, or docs.
- [ ] CI brings obs up by default and obs tests pass in the relevant jobs.
- [ ] `dev-doctor.sh` validates the new fixture; docs describe the always-on model and the solution-agnostic app boundary.

## Dependencies

- WP01–WP04 (the new fixture and tests must be in place before the old stack is removed and CI flips).
