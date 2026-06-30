---
verblock: "30 Jun 2026:v0.1: vscode - Initial version"
intent_version: 2.4.0
status: Not Started
slug: clickhouse-observability-as-an-always-on-compose
created: 20260630
completed:
---

# ST0027: ClickHouse observability as an always-on compose fixture

## Objective

Replace the standalone `projects/observability/` stack (built in ST0026: OTel Collector + Tempo + Prometheus + Loki + Vector + Grafana) with a single ClickHouse-backed OpenTelemetry pipeline folded into `projects/compose/` as an **always-on dev/CI fixture** — present by default like PostgreSQL and Hydra. Observability tests stop skipping: they always run and **fail** if the fixture is absent, exactly like the Hydra-dependent tests. The application code stays **solution-agnostic** (it only ever emits OTLP to a configurable endpoint), so users can point it at their own OTel backend unchanged.

## Context

ST0026 delivered OpenTelemetry traces/metrics/logs end-to-end, but the local backend was a six-service Grafana stack run as its own compose project with a bespoke external-network attach dance (`up.sh`/`down.sh`), dynamic `OBS_HOST_DIR`/`OBS_NETWORK` translation, and an `obs_stack_ready()` skip guard so tests were best-effort. That stack was deliberately *opt-in* and excluded from CI, which left the observability tests effectively unenforced.

A hands-on spike (branch `spike/clickstack-observability`, see `FINDINGS.md` on that branch — it is not on this branch) evaluated ClickStack/HyperDX. Key finding: the **all-in-one** image is control-plane-driven (OpAMP supervisor + MongoDB), requires first-run setup, and gates OTLP behind a per-team bearer token — a poor fit for a deterministic, git-tracked, always-on fixture. The **modular** approach dissolves all three problems: we keep our own statically-configured **stock `otel/opentelemetry-collector-contrib`** collector (the `clickhouse` exporter ships in contrib), terminate TLS on the OTLP receiver ourselves (keyless, like today), and use **ClickHouse** as the unified store (replacing Tempo + Prometheus + Loki). **HyperDX** rides on top purely as a reader UI (with MongoDB for its own state); tests query ClickHouse directly via SQL and never need it.

Because we own the collector, the ClickStack "auth header" peculiarity never reaches the application — the app keeps sending plain OTLP over TLS with no `Authorization` header and no ClickStack-specific config. A generic `otlp_headers` option is added to `udex-telemetry` anyway, as an agnostic convenience for users whose own backends require header auth (Honeycomb, Grafana Cloud, ClickStack all-in-one).

Decisions taken at planning (see design.md): the full backend (collector + ClickHouse + HyperDX + Mongo) is **always-on everywhere including CI**; obs gets a **non-k8s test path** in the main `cargo test` job (in addition to the existing k8s path) so it runs unconditionally; the container-stdout "log floor" is **folded into the collector** (filelog/docker receiver), retiring the separate Vector service.

## Related Steel Threads

- ST0026 (Open Observability) — built the original six-service stack this thread replaces; the application-side OTLP instrumentation (server/SDK/CLI, `udex-telemetry`) is reused unchanged.

## Context for LLM

This document represents a single steel thread - a self-contained unit of work focused on implementing a specific piece of functionality. When working with an LLM on this steel thread, start by sharing this document to provide context about what needs to be done.

### How to update this document

1. Update the status as work progresses
2. Update related documents (design.md, impl.md, etc.) as needed
3. Mark the completion date when finished

The LLM should assist with implementation details and help maintain this document as work progresses.
