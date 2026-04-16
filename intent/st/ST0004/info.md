---
verblock: "16 Apr 2026:v0.1: vscode - Initial version"
intent_version: 2.4.0
status: WIP
slug: benchmark-testing
created: 20260416
completed:
---

# ST0004: Benchmark testing

## Objective

Establish a repeatable benchmark suite for the Udex server, focused on the Entry API (create, get, bulk write, bulk read). The suite must be runnable locally and in CI, and must produce statistically meaningful results that can detect regressions over time.

## Context

The Entry API is the primary hot path for Udex. Before optimising it we need a baseline. Benchmarks must be realistic — running against a real PostgreSQL instance with the full gRPC stack — rather than microbenchmarks of isolated functions.

### Framework decision: Criterion

After evaluating `libtest`, `criterion`, and `divan`:

| Framework | Async | Stats | CI output | Stable |
|-----------|-------|-------|-----------|--------|
| libtest | workaround needed | minimal | poor | nightly only |
| **criterion** | **native** | **confidence intervals, regression detection** | **HTML + JSON** | **yes** |
| divan | native | percentiles | terminal-focused | yes |

**Criterion** is the choice. Key reasons:
- DB operations introduce significant noise; Criterion's confidence intervals and outlier detection separate signal from variance reliably
- HTML reports and JSON export give traceable, shareable results across runs
- Mature ecosystem with good tokio integration
- Reuses existing test fixtures (PostgresDatastore, index initialisation)

## Scope

Two benchmark layers — the datastore is expected to be the primary bottleneck and is benchmarked independently to separate DB overhead from gRPC/server overhead:

**Datastore layer (`udex-datastore`)**
- Direct `PostgresDatastore` method calls (no gRPC overhead)
- Entry operations: `create_entry`, `get_entry_by_key`, `get_entries_by_context`, `delete_entry`
- Bulk operations: `bulk_write`, `bulk_read` at N = 10, 100, 1000
- Isolates query and connection pool performance

**Server / gRPC layer (`udex-server`)**
- Full gRPC stack end-to-end (client → server → datastore → DB)
- Same operation set as above
- Measures the overhead added by gRPC, auth, and middleware on top of the datastore baseline

**Out of scope**
- Index API (admin path, not hot path)
- Connection pool tuning (a separate optimisation concern)

## Related Steel Threads

- ST0001 (Structured logging) — logging overhead may be visible in benchmarks
