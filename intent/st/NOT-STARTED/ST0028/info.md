---
verblock: "08 Aug 2026:v0.1: vscode - Initial version"
intent_version: 2.15.0
status: Not Started
slug: openobserve-as-the-dev-observability-backend
created: 20260808
completed:
---

# ST0028: OpenObserve as the dev observability backend

## Objective

Replace the ClickHouse + HyperDX + MongoDB half of the always-on observability fixture with a single **OpenObserve** service backed by local disk, keeping the OpenTelemetry Collector and the Vector log floor. The fixture goes from five services to three while preserving every capability it has today: OTLP ingest of traces, metrics, and logs; a browsable developer UI; a queryable store the integration tests assert against; and the postgres/hydra container-log floor sitting alongside application telemetry.

The application stays **solution-agnostic**. `udex-telemetry` continues to emit plain OTLP to a configurable endpoint, so no production crate changes in this thread — the backend swap is invisible above the Collector.

## Context

ST0027 folded observability into `projects/compose/` as an always-on dev/CI fixture. It works, but the backend half is heavy for something that only ever serves local development: ClickHouse (with a raised `nofile` ulimit and a `CLICKHOUSE_DB` bootstrap), HyperDX as a reader-only UI, MongoDB purely to hold HyperDX's own state, and a `hyperdx-init` one-shot that registers a local user so a developer never meets a signup form. Five services, a ~1.5KB inline `DEFAULT_SOURCES` JSON blob, and two moving parts (Mongo, hyperdx-init) that exist only to make a UI usable.

OpenObserve collapses store, query API, and UI into one binary with a SQL-over-HTTP search API. In local mode it keeps metadata in SQLite and segments on local disk, so it needs no companion database and no bootstrap.

A hands-on spike (branch `spike/openobserve-obs`, `FINDINGS.md` on that branch — it is not on this branch) proved the approach end to end before this thread was planned. It stood the stack up beside the running ClickHouse fixture and drove the **real** `udex-sdk` obs test at it, then recorded the schema and the query shape for every existing assertion. Verdict: viable, no blockers. All four existing observability assertions have verified translations, and the trace query ports essentially verbatim.

The spike also surfaced the things that will actually cost time, all of which this thread plans for explicitly rather than discovers late:

- OpenObserve prefixes resource attributes with `service_` in the **traces** stream but not in logs or metrics, so a mistyped column is easy to write. Rejected queries carry their reason in a `message` field (plus often a `hint`), which a naive helper discards. Surfacing that reason is an acceptance criterion, not a nicety — without it a typo is indistinguishable from telemetry never arriving, which is a `IN-AG-NO-SILENT-001` violation.
- `max_by` is absent from OpenObserve's DataFusion build, so the existing `argMax(Value, TimeUnix)` metric query has no direct translation. `max(value)` per series is correct **because** `udex.rpc.requests` is a monotonic cumulative counter; that reasoning has to be written down next to the query or it reads as a bug.
- Vector 0.44's `opentelemetry` sink is thin: it needs the OTLP envelope hand-built in VRL, an explicit `Content-Type`, and one event per request.

Storage is **local disk, not MinIO**. Object storage was considered and rejected: the driver for this thread is simplification, and MinIO would have added back a stateful service plus bucket bootstrap to a fixture whose whole point is being light. The object-storage path is not part of what Udex needs to exercise — the OTel capability under test is the app's OTLP emission, which is unaffected by how the backend persists segments.

Sequencing is deliberately **strangler-style**: OpenObserve is stood up beside ClickHouse and the Collector dual-exports, so every work package leaves the tree green. ClickHouse is retired only once the tests read from OpenObserve. This keeps trunk-based development honest — no work package depends on a red window closing.

## Acceptance

Acceptance Criteria and Acceptance Tests for this steel thread live in `acceptance.md` (the single source of truth). Do not restate ACs here -- see that file for the ratified completeness boundary and live status.

## Related Steel Threads

- ST0026 (Open Observability) — built the original Grafana-stack observability and the application-side OTLP instrumentation (`udex-telemetry`, server/SDK/CLI) that this thread reuses unchanged.
- ST0027 (ClickHouse observability as an always-on compose fixture) — established the always-on, fail-never-skip fixture posture and the bind-mount-free inline-config constraint. This thread replaces its backend half and inherits its rules.

## Context for LLM

This document represents a single steel thread - a self-contained unit of work focused on implementing a specific piece of functionality. When working with an LLM on this steel thread, start by sharing this document to provide context about what needs to be done.

### How to update this document

1. Update the status as work progresses
2. Update related documents (design.md, impl.md, etc.) as needed
3. Mark the completion date when finished

The LLM should assist with implementation details and help maintain this document as work progresses.
