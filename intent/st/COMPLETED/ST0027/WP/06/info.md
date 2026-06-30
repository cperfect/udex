---
verblock: "30 Jun 2026:v0.1: vscode - Initial version"
wp_id: WP-06
title: "Decommission projects/observability + docs + CI"
scope: Small
status: Done
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

- [x] No references to `projects/observability/` remain in code, scripts, CI, or docs. (Dir deleted; live-file grep clean — only the ST planning docs mention it as history.)
- [x] CI brings obs up by default and obs tests pass in the relevant jobs. (`test` + `k8s-test` start the obs services from the base compose with localhost addressing; `obs.rs` runs in `test`, `test_obs_k8s_*` in `k8s-test`. The k8s obs tests were validated live: 3/3 pass after a plaintext redeploy. The HyperDX UI is omitted from CI by design — user-confirmed.)
- [x] `dev-doctor.sh` validates the new fixture; docs describe the always-on model and the solution-agnostic app boundary. (dev-doctor drops the obsolete OTLP-cert/Grafana checks; new compose-level obs README + ARCHITECTURE/SECRETS/CONTRIBUTING/devcontainer docs updated.)

## Dependencies

- WP01–WP04 (the new fixture and tests must be in place before the old stack is removed and CI flips).
