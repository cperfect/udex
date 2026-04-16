---
verblock: "16 Apr 2026:v0.1: vscode - Initial version"
wp_id: WP-05
title: "Datastore layer benchmarks"
scope: Small
status: Not Started
---

# WP-05: Datastore layer benchmarks

## Objective

Benchmark `PostgresDatastore` directly — bypassing the gRPC and server layers — to establish a pure DB performance baseline. This isolates query execution and connection pool overhead from the gRPC stack, making it easier to attribute latency to the correct layer.

## Deliverables

- `projects/rust/datastore/Cargo.toml` — `[[bench]]` entry, `criterion` dev-dependency
- `projects/rust/datastore/benches/postgres_datastore.rs` — benchmarks for:
  - `create_entry`
  - `get_entry_by_key`
  - `get_entries_by_context`
  - `delete_entry`
  - `bulk_write` at N = 10, 100, 1000
  - `bulk_read` at N = 10, 100, 1000
- Shared setup helpers (reuse `integration_test` helpers for DB provisioning)

## Acceptance Criteria

- [ ] All entry operations benchmarked at the datastore level
- [ ] Bulk benchmarks report throughput (entries/sec) via `criterion.throughput()`
- [ ] Results directly comparable to WP-02/WP-03 gRPC benchmarks (same operation set, same N values)
- [ ] DB cleanup on teardown (same pattern as integration tests)
- [ ] `cargo bench --no-run` passes in CI

## Notes

Run WP-05 (datastore) alongside WP-02/WP-03 (gRPC) to quantify the overhead added by the server and transport layer. Expect the gRPC benchmarks to show 20-50% higher latency than the datastore benchmarks for simple operations.

## Dependencies

- WP-01 (harness patterns — reuse setup/teardown approach)
