---
verblock: "20 May 2026:v0.1: Chris Perfect - Initial version"
intent_version: 2.4.0
status: Completed
slug: grpc-native-health-check
created: 20260520
completed: 20260520
---

# ST0019: gRPC native health check

## Objective

Replace the custom `udex.healthz.v1` health check service with the standard gRPC Health Checking Protocol (`grpc.health.v1.Health`). This enables native gRPC health probes in Kubernetes and aligns with ecosystem tooling (grpc-health-probe, Envoy, Istio, k8s gRPC probes).

## Context

Udex currently implements a custom `HealthzService` backed by a bespoke `udex.healthz.v1` proto. The gRPC ecosystem has a well-established standard: https://github.com/grpc/grpc/blob/master/doc/health-checking.md. The standard defines:

- Service: `grpc.health.v1.Health`
- Methods: `Check` (unary) and `Watch` (server-streaming)
- Per-service health status: `SERVING`, `NOT_SERVING`, `SERVICE_UNKNOWN`

The Rust/tonic ecosystem provides `tonic-health` which implements this protocol out of the box. Adopting it:

- Removes the custom proto and its generated code
- Exposes per-service health status (entry, index, overall server)
- Resolves the TODO in `projects/k8s/helm/udex/templates/deployment.yaml` to switch from `tcpSocket` probes to native gRPC probes
- Works with standard tooling without any Udex-specific client code

The change touches: the `api` crate (proto removal), `server` crate (HealthzService, HealthCheck trait, server wiring), all test fixtures that poll healthz to detect server readiness (cli tests, server integration tests, sdk tests), and the Helm chart.

## Scope

### In

- Add `tonic-health` to the `server` crate
- Implement per-service health reporting: register `""` (overall), `"udex.entry.v1.EntryService"`, and `"udex.index.v1.IndexService"` with the health reporter
- Wire the health reporter into `EntryService` and `IndexService` so that status transitions to `NOT_SERVING` when the datastore becomes unhealthy
- Remove `projects/protobuf/udex.healthz.v1.proto` and its generated code
- Remove `server/src/healthz.rs` and the `HealthzService` / `HealthCheck` trait
- Update all test fixtures that use `HealthzServiceClient` to use `tonic_health::pb::health_client::HealthClient`
- Update the Helm `deployment.yaml` liveness and readiness probes from `tcpSocket` to `grpc`
- Update the Helm `values.yaml` and any probe-related Helm template helpers
- Update docs (server/README.md, k8s/README.md, CONTRIBUTING.md)

### Out

- No changes to the entry, index, or auth business logic
- No changes to the CLI binary surface (there is no `udex health` command)
- No changes to the datastore crate

## Work Packages

- WP01: Add `tonic-health`, wire health reporter into server, remove custom healthz
- WP02: Update all test fixtures (server, cli, sdk) to use standard health client
- WP03: Update Helm chart probes and docs

## Related Steel Threads

- ST0018: local-k8s-and-helm-dev (introduced the tcpSocket probe TODO this resolves)

## Context for LLM

### Current state

- Custom proto: `projects/protobuf/udex.healthz.v1.proto`
- Generated code: `projects/rust/api/src/generated/udex.healthz.v1.rs`
- Server implementation: `projects/rust/server/src/healthz.rs` (`HealthzService`)
- `HealthCheck` trait defined in `projects/rust/server/src/lib.rs` (implemented by `EntryService` and `IndexService`)
- Server wires `HealthzServiceServer` in `projects/rust/server/src/server.rs`
- Tests using `HealthzServiceClient` for readiness polling:
  - `projects/rust/cli/tests/serve_live_tests.rs` (primary server readiness poll)
  - `projects/rust/cli/tests/entry_live_tests.rs`
  - `projects/rust/cli/tests/index_oauth2_tests.rs`
  - `projects/rust/cli/tests/token_oauth2_tests.rs`
  - `projects/rust/server/tests/server_integration_tests.rs`
- Helm TODO: `projects/k8s/helm/udex/templates/deployment.yaml` lines 84-100 (tcpSocket probes, TODO comment references this spec)

### Target state

- `tonic-health` added to `server` crate (and `tonic-health` client in test crates via dev-deps)
- `HealthReporter` created at server startup; passed into `EntryService` and `IndexService` so they can flip their status
- Services registered: `""` (whole server), `"udex.entry.v1.EntryService"`, `"udex.index.v1.IndexService"`
- `udex.healthz.v1` proto, generated code, `HealthzService`, and `HealthCheck` trait all deleted
- Test readiness polling switches to `tonic_health::pb::health_client::HealthClient` checking `""` service
- Helm probes:
  ```yaml
  livenessProbe:
    grpc:
      port: 443
      service: ""
  readinessProbe:
    grpc:
      port: 443
      service: "udex.entry.v1.EntryService"
  ```
  Note: k8s gRPC probes require k8s 1.24+ and do not support TLS natively before 1.31 -- verify cluster version and consider whether to use `grpc-health-probe` as a fallback exec probe.
