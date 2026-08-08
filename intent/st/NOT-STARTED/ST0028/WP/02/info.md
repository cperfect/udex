---
verblock: "08 Aug 2026:v0.1: vscode - Initial version"
wp_id: WP-02
title: "Port the observability verification layer to OpenObserve"
scope: Small
status: Done
---

# WP-02: Port the observability verification layer to OpenObserve

## Objective

Repoint every existing observability assertion from ClickHouse SQL to OpenObserve's search API, keeping the assertions themselves semantically identical. No new coverage here -- that is WP03. At the end of this work package the tests read from OpenObserve and ClickHouse is unread, which is what makes WP04's deletion safe.

## Deliverables

- New `openobserve_*` helpers in `projects/rust/sdk/tests/common/mod.rs` replacing the four `clickhouse_*` helpers: one `search(sql, stream_type)` primitive plus the polling wrappers that mirror today's `_count` / `_scalar_f64` / `_trace_span_names` behaviour. Base URL and Basic-auth credential read from env with in-network service-name defaults, exactly as `CLICKHOUSE_URL` works today.
- `sdk/tests/obs.rs` retargeted: three assertions plus the module doc comment, which currently narrates the ClickHouse path.
- `integration_tests.rs` retargeted: the three `test_obs_k8s_*` tests and their eight `clickhouse` references.

## Implementation notes

**The search helper must surface API errors.** A rejected query is answered with **HTTP 400** carrying `{"code": ..., "message": "unknown field '...'", "hint": "..."}`; some conditions instead answer 200 with `message` set and `hits` null. Handle both, and — crucially — read the body *before* reacting to the status, because `error_for_status()` alone discards the `message` and `hint` that name the offending column and suggest the repair. `IN-AG-NO-SILENT-001` makes this mandatory. This single decision is what makes the prefix asymmetry below survivable.

(The spike recorded this as "HTTP 200 with the error in the body" — it had inspected the body with `jq` and never checked the status code. Corrected during implementation.)

**Resource attributes are prefixed inconsistently by signal** -- `service_` in traces only. `udex.test.run` is `service_udex_test_run` when querying traces but `udex_test_run` in logs and metrics; `service.instance.id` is `service_service_instance_id` in traces but `service_instance_id` in metrics. This is the most likely source of a confusing failure in the whole thread.

**Query API shape:** `POST /api/{org}/_search?type={logs|traces|metrics}` with `{"query": {"sql", "start_time", "end_time", "from", "size"}}`. Two traps: the times are **microseconds** and mandatory, and a window that does not bracket the data returns zero hits with no error; and the stream name `default` exists independently under both `logs` and `traces`, so an omitted `type` silently searches the wrong signal.

**Metric query.** `max_by` does not exist in OpenObserve's DataFusion build, so `argMax(Value, TimeUnix)` has no direct port. Use `max(value)` per series -- correct **because** `udex.rpc.requests` is a monotonic cumulative counter, so the latest value per series is also its maximum. That precondition must appear as a comment beside the query or it reads as a bug. Metrics live in one stream per metric name with dots as underscores (`udex_rpc_requests`).

**Trace query ports essentially verbatim** -- the `IN` subquery works. Note the span attribute is `key` (needs SQL quoting), matching today's `SpanAttributes['key']`; it is not a namespaced `udex.entry.key`. The spike confirmed this against real telemetry after synthetic data suggested otherwise.

Preserve ST0027's fail-never-skip posture: helpers fail when the backend is unreachable, exactly like the Hydra-dependent tests.

## Acceptance

Acceptance Criteria for this work package live in the steel thread's `acceptance.md`, under the `WP-02` heading (single source of truth). Do not restate ACs here.

## Dependencies

- WP-01 (OpenObserve must be receiving telemetry before anything can assert against it).
- Reference: `FINDINGS.md` on branch `spike/openobserve-obs` -- full schema map and every query verified against real `udex-server` telemetry.
