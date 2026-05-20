---
verblock: "20 May 2026:v0.1: Chris Perfect - Initial version; v0.2: Chris Perfect / Claude - As-built update"
wp_id: WP-03
title: "Update Helm chart probes and docs"
scope: Small
status: Done
---

# WP-03: Update Helm chart probes and docs

## Objective

Investigate and resolve the TODO comment in the Helm chart regarding gRPC health probes. Document the probe approach and update relevant documentation.

> **As-built note:** The original objective assumed `tcpSocket` probes would be replaced with native `grpc` probes. Investigation confirmed this is not possible: native k8s `grpc` probes (beta/default since 1.24, GA since 1.27) make a plain non-TLS connection. The server is TLS-only on port 443, so native probes would fail at the TLS handshake. `tcpSocket` probes are retained. The TODO comment was replaced with an explanation of the constraint and the future path.

## Deliverables (as-built)

- `projects/k8s/helm/udex/templates/deployment.yaml`: replaced the vague TODO comment with a precise explanation of:
  - why `tcpSocket` probes are retained (native `grpc` probes do not support TLS)
  - what `tcpSocket` actually checks (TCP connect only — no TLS handshake, no gRPC)
  - the exec-based `grpc-health-probe --tls` path for full TLS + gRPC validation
  - the future path (non-TLS health port enables native `grpc` probes without extra binary)
- `projects/k8s/README.md`: new "Health probes" section covering registered services, the TLS constraint, the exec-based alternative, and how to probe manually
- `helm lint --strict` passes

## Acceptance Criteria (as-built)

- [x] The TODO comment referencing the gRPC health checking spec is removed
- [x] `deployment.yaml` accurately describes what `tcpSocket` probes check (TCP connect only)
- [x] `projects/k8s/README.md` documents the probe decision and the TLS constraint
- [x] `bash scripts/validate-lint-helm.sh` passes

## Dependencies

- WP01 must be complete (server must expose `grpc.health.v1.Health` before probes can use it)
- WP02 is independent of this WP
