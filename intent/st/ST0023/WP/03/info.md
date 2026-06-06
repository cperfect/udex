---
verblock: "06 Jun 2026:v0.1: vscode - Initial version"
wp_id: WP-03
title: "Update k8s deploy path to config.yaml"
scope: Small
status: Done
---

# WP-03: Update k8s deploy path to config.yaml

## Objective

Update the Kubernetes deploy path to render and mount the server config as YAML instead of TOML, matching the new loader.

## Deliverables

- `projects/k8s/helm/udex/templates/configmap.yaml` rewritten: render `config.yaml` (nested YAML mappings) instead of `config.toml` (`[tables]`), keeping the `urn:secrets-rs:…` references verbatim.
- `projects/k8s/helm/udex/templates/deployment.yaml` mount paths updated (`/etc/udex/config.toml` → `/etc/udex/config.yaml`, `subPath`, and the comments at lines ~66–109).
- `values.yaml` inputs unchanged (only the rendered body/filename change).

## Acceptance Criteria

- [x] `helm template` / `helm lint` produce a valid `config.yaml` ConfigMap; `bash scripts/validate-lint-helm.sh` passes.
- [x] A k3d deploy starts the server, which loads the mounted `config.yaml` successfully (`bash scripts/validate-k8s-test.sh` — deployment rolled out + all 6 `test_sdk_k8s_*` tests pass over TLS).

## Dependencies

- **WP-02** (YAML config loader exists).
