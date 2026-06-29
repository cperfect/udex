---
verblock: "24 Jun 2026:v0.1: vscode - Initial version"
intent_version: 2.4.0
status: Completed
slug: open-observability
created: 20260624
completed: 20260628
---

# ST0026: Open Observability

## Objective

Make Udex fully observable by modern open standards (OpenTelemetry): distributed
tracing, metrics, and log aggregation for the server-side (application + the
PostgreSQL datastore). Provide a local Grafana-based stack (Grafana, Prometheus,
Grafana Loki, Grafana Tempo, Vector, OpenTelemetry Collector) for development,
and keep the Udex application coupled only to open standards (OTel) so it is
never locked to a specific backend technology.

This serves two goals:

1. **User feature** - Udex is observable in a user's environment by plugging the
   OTLP signals into any OTel-compatible backend.
2. **Development capability (immediate)** - we can use observability to develop
   and tune Udex, especially for performance, stress, and reliability work.

## Context

The server currently emits JSON logs to stdout via `tracing` /
`tracing-subscriber` and has a `tower_http` gRPC `TraceLayer`, but there is no
OpenTelemetry, no metrics, and no datastore query instrumentation. The SDK has
`tracing` as a dependency but does not use it. There is no telemetry config
section and no local observability backend.

This steel thread introduces an end-to-end OpenTelemetry pipeline and the local
runtime stack to receive it, without coupling application code to any specific
observability vendor.

### Key design decisions

- **Open-standard boundary** - a new `udex-telemetry` crate owns all OTel
  SDK/exporter setup for *binaries* (server, CLI): OTLP traces + metrics + logs,
  sampling, resource attributes, graceful disable. The **SDK never owns a
  provider** - it only emits `tracing` spans and propagates W3C `traceparent`,
  so it integrates into a host application's existing OTel context.
- **Logs - hybrid** - the app emits logs over OTLP when an endpoint is
  configured, and *always* writes JSON to stdout as a durable floor. Trace/log
  correlation comes from the OTLP path; crash/Collector-outage durability comes
  from stdout (scraped by Vector).
- **Metrics - OTLP push** - the app pushes metrics via OTLP to the Collector,
  which exposes/forwards them to Prometheus. No second listener is added; the
  server stays single-port TLS-gRPC.
- **Config - schema first** - a new optional `observability` section in
  `ServerConfig` (serde/YAML): `enabled`, `otlp_endpoint`, `sample_ratio`,
  per-signal toggles, resource attributes. Defaults to disabled; validated
  alongside the existing TLS/authz sections. No protobuf change (this is server
  config, not the gRPC API).
- **TLS on OTLP** - consistent with the project's TLS-everywhere principle,
  app->Collector OTLP uses TLS with generated, gitignored certs (same pattern as
  the pod/edge certs).
- **Local dev - opt-in** - observability components are layered into the dev
  stack but off by default (compose profile + up/down/rebuild script); app
  telemetry auto-disables when no OTLP endpoint is configured. K8s dev
  deployments run with full trace + metric sampling.
- **PostgreSQL** - DB server metrics via the Collector `postgresqlreceiver`;
  client-side query spans via sqlx tracing.

## Related Steel Threads

- ST0024 (K8s ingress TLS) - established the edge/pod cert pattern reused for
  OTLP TLS.
- ST0025 (Cluster integration tests) - established the multi-instance k8s test
  harness this thread's k8s observability tests build on.

## Context for LLM

This document represents a single steel thread - a self-contained unit of work focused on implementing a specific piece of functionality. When working with an LLM on this steel thread, start by sharing this document to provide context about what needs to be done.

### How to update this document

1. Update the status as work progresses
2. Update related documents (design.md, impl.md, etc.) as needed
3. Mark the completion date when finished

The LLM should assist with implementation details and help maintain this document as work progresses.
