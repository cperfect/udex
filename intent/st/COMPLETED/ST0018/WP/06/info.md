---
verblock: "18 May 2026:v0.1: vscode - Initial version"
wp_id: WP-06
title: "CI job (k8s-test in 01-Validation.yml)"
scope: Small
status: Done
---

# WP-06: CI job (k8s-test in 01-Validation.yml)

## Objective

Add a `k8s-test` job to `.github/workflows/01-Validation.yml` that spins up a k3d cluster, deploys via Helm, runs the `test_sdk_k8s_*` tests, and always tears down — gated on relevant path changes.

## Deliverables

- `.github/workflows/01-Validation.yml` — two new jobs: `changes` (path filter) and `k8s-test`.

## Design notes

- `changes` job uses `dorny/paths-filter@v3` to detect relevant file changes; outputs a `k8s` boolean.
- `k8s-test` runs only when `changes.k8s == 'true'` (paths: `projects/k8s/**`, `projects/rust/cli/Dockerfile`, `projects/rust/cli/src/**`, `projects/rust/sdk/tests/**`).
- Installs k3d v5.8.3, kubectl v1.36.1, helm v4.0.0 (matching devcontainer versions).
- Writes a minimal `.env` from CI secrets so `deploy.sh` can source `POSTGRES_PASSWORD_SECRET`.
- Teardown step uses `if: always()` to guarantee cluster cleanup on failure.

## Acceptance Criteria

- [x] `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/01-Validation.yml'))"` passes (valid YAML)
- [x] New jobs follow the existing workflow style (delegate to scripts where possible)
- [x] Teardown is unconditional (`if: always()`)

## Dependencies

- WP01–05 — all prior work packages must be in place for the job to function
