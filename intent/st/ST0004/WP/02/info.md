---
verblock: "16 Apr 2026:v0.1: vscode - Initial version"
wp_id: WP-02
title: "Single-entry operation benchmarks"
scope: Small
status: Done
---

# WP-02: Single-entry operation benchmarks (gRPC layer)

## Objective

Benchmark each individual Entry API operation at single-entry scale via the full gRPC stack to establish per-operation end-to-end baselines. Compare against WP-05 (datastore layer) to quantify gRPC and server overhead.

## Deliverables

- `projects/rust/server/benches/entry_service.rs` — benchmarks for:
  - `create_entry` — create one entry
  - `get_entry_by_key` — lookup by key (entry exists)
  - `get_entries_by_context` — lookup by context (single result)
  - `delete_entry` — delete one entry

## Acceptance Criteria

- [ ] All four operations benchmarked
- [ ] Each benchmark uses a fresh entry per iteration (no cross-contamination)
- [ ] Results are stable enough for Criterion to report a mean and std dev (not `high noise` warnings)
- [ ] Benchmark names follow convention: `entry/<operation>` (e.g. `entry/create`, `entry/get_by_key`)

## Dependencies

- WP-01 (harness)
