---
verblock: "18 May 2026:v0.1: vscode - Initial version"
wp_id: WP-03
title: "Cluster management and deploy scripts"
scope: Small
status: Done
---

# WP-03: Cluster management and deploy scripts

## Objective

Provide shell scripts under `projects/k8s/scripts/` to manage the full local k3d lifecycle: cluster creation, image build and load, Helm deploy/undeploy, and cluster teardown.

## Deliverables

- `cluster-create.sh` — idempotent k3d cluster create with `--port 8443:443@loadbalancer`
- `cluster-delete.sh` — k3d cluster delete (idempotent)
- `image-build.sh` — docker build with correct context; accepts `--dev` for a debug build
- `image-load.sh` — k3d image import into the cluster
- `deploy.sh` — helm upgrade --install; reads `.env` for credentials, test certs for TLS; waits for rollout
- `undeploy.sh` — helm uninstall (idempotent)

## Acceptance Criteria

- [x] All six scripts pass `bash -n` (syntax check)
- [x] All scripts are executable (`chmod +x`)
- [x] All scripts use `set -euo pipefail` and check prerequisites

## Dependencies

- WP01 (Dockerfile) — image-build.sh references it
- WP02 (Helm chart) — deploy.sh references the chart
