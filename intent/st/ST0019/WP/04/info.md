---
verblock: "20 May 2026:v0.1: Chris Perfect - Initial version"
wp_id: WP-04
title: "Documentation and FAQ"
scope: Small
status: Not Started
---

# WP-04: Documentation and FAQ

## Objective

Update all project documentation to reflect the removal of the custom healthz API and add a FAQ entry explaining why the standard gRPC health checking protocol was adopted.

## Deliverables

- `docs/FAQ.md`: add a new entry "Why does Udex use the gRPC Health Checking Protocol instead of a custom healthz endpoint?" covering:
  - The standard (`grpc.health.v1.Health`) is understood by Kubernetes, Envoy, Istio, grpc-health-probe, and any gRPC-aware load balancer without Udex-specific configuration
  - Per-service granularity (`EntryService`, `IndexService`) is built in; a custom proto would duplicate this concern
  - Enables native k8s `grpc` probes (replacing `tcpSocket`), which actually exercise the gRPC stack rather than just checking whether the port is open
  - The custom `udex.healthz.v1` proto was bespoke with no tooling support outside this repo

- `docs/ARCHITECTURE.md`: update any references to the custom healthz service; describe the standard health endpoint and which services are registered

- `projects/rust/server/README.md`:
  - Remove the `healthz.rs` row from the module table
  - Update the auth section (currently says "every gRPC request except `/healthz`") to reference `grpc.health.v1.Health`

- `projects/rust/api/README.md`:
  - Remove the `healthz (udex.healthz.v1)` row from the module table

- `projects/k8s/README.md`: describe the gRPC probe setup and note the k8s version requirement (1.24+ for GA gRPC probes; TLS caveat if applicable)

## Acceptance Criteria

- [ ] `docs/FAQ.md` contains the new health check FAQ entry
- [ ] No doc refers to `udex.healthz.v1`, `HealthzService`, or the custom `Healthz` RPC
- [ ] `projects/rust/server/README.md` auth section correctly describes the unauthenticated endpoint as `grpc.health.v1.Health`

## Dependencies

- Best done after WP01-03 are complete so the as-built state is known, but can be drafted in parallel
