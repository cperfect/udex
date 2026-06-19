---
verblock: "19 Jun 2026:v0.1: vscode - Initial version"
wp_id: WP-01
title: "Chart: default dev to 2 replicas"
scope: Small
status: Done
---

# WP-01: Chart: default dev to 2 replicas

## Objective

Make the server replica count configurable and default the dev deployment to 2 replicas.

## Deliverables

- `values.yaml`: `replicaCount: 2` (commented — dev default for multi-instance coverage).
- `templates/deployment.yaml`: `replicas: {{ .Values.replicaCount }}`; header comment updated.

(Doc updates are consolidated in WP05.)

## Acceptance Criteria

- [x] `helm template ... ` renders a Deployment with `replicas: 2` by default and honours `--set replicaCount=N` (verified 2 default, 3 via `--set`)
- [x] `bash scripts/validate-lint-helm.sh` passes

## Dependencies

- None (foundational; WP02+ deploy against it).
