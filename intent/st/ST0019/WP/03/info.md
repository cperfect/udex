---
verblock: "20 May 2026:v0.1: Chris Perfect - Initial version"
wp_id: WP-03
title: "Update Helm chart probes and docs"
scope: Small
status: Not Started
---

# WP-03: Update Helm chart probes and docs

## Objective

Replace the `tcpSocket` liveness and readiness probes in the Helm chart with native gRPC probes, resolving the TODO introduced in ST0018. Update all relevant documentation.

## Deliverables

- `projects/k8s/helm/udex/templates/deployment.yaml`: replace `tcpSocket` probes with `grpc` probes
  - `livenessProbe`: check `""` (overall server)
  - `readinessProbe`: check `"udex.entry.v1.EntryService"` (gates traffic until entry service is ready)
  - Remove the TODO comment
- Verify the minimum k8s version required for gRPC probes (1.24 GA). Document in `projects/k8s/README.md` if it differs from what the chart already declares.
- Consider whether TLS matters for probes: k8s gRPC probes before 1.31 do not support TLS. If the cluster uses TLS-only (as the current k3d setup does on port 443), an exec-based `grpc-health-probe` may be needed as an interim. Investigate and document the decision.
- Update `projects/k8s/README.md` to describe the health probe approach
- Update `CONTRIBUTING.md` k8s section if relevant
- `bash scripts/validate-lint-helm.sh` passes (helm lint --strict)
- `bash scripts/validate-k8s-test.sh` passes end-to-end

## Acceptance Criteria

- [ ] No `tcpSocket` probe remains in `deployment.yaml`
- [ ] `bash scripts/validate-lint-helm.sh` passes
- [ ] `bash scripts/validate-k8s-test.sh` passes (pod reaches Ready, traffic flows)
- [ ] The TODO comment referencing the gRPC health checking spec is removed

## Dependencies

- WP01 must be complete (server must expose `grpc.health.v1.Health` before probes can use it)
- WP02 is independent of this WP
