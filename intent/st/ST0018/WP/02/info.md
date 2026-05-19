---
verblock: "18 May 2026:v0.1: vscode - Initial version"
wp_id: WP-02
title: "Helm chart and k8s manifests"
scope: Small
status: Done
---

# WP-02: Helm chart and k8s manifests

## Objective

Produce a Helm chart under `projects/k8s/helm/udex/` that deploys the `udex` container with OAuth2-only auth, TLS, and external Compose services (Postgres + Hydra) reachable via `host.k3d.internal`.

## Deliverables

- `projects/k8s/helm/udex/Chart.yaml`
- `projects/k8s/helm/udex/values.yaml`
- `projects/k8s/helm/udex/templates/_helpers.tpl`
- `projects/k8s/helm/udex/templates/configmap.yaml` (renders `UdexConfig` as TOML)
- `projects/k8s/helm/udex/templates/secret.yaml` (DATABASE_URL, tls.crt, tls.key)
- `projects/k8s/helm/udex/templates/deployment.yaml`
- `projects/k8s/helm/udex/templates/service.yaml` (LoadBalancer, port 443)

## Acceptance Criteria

- [x] `helm lint projects/k8s/helm/udex` passes with 0 failures

## Dependencies

- WP01 (Dockerfile) — image must exist to deploy
