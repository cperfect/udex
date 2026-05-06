---
verblock: "06 May 2026:v0.1: vscode - Initial version"
wp_id: WP-01
title: "Capture benchmark baseline on current schema"
scope: Small
status: Not Started
---

# WP-01: Capture benchmark baseline on current schema

## Objective

Fix the `bench_create_entry` benchmark to use a unique context per iteration (the current shared-context pattern reuses the same context across iterations, making all but the first a no-op under 1:1). Once fixed, capture a named Criterion baseline on the current two-table schema so there is a valid before/after comparison point for WP-06.

## Deliverables

- Updated `bench_create_entry` that generates a unique context per iteration (e.g. incrementing counter in pairs)
- Criterion baseline captured with `--save-baseline two-table` against the current schema
- Baseline file committed or recorded (git SHA noted in impl.md)

## Acceptance Criteria

- [ ] `bench_create_entry` no longer reuses the same context across iterations
- [ ] `cargo bench --bench entry_service -- --save-baseline two-table` completes without error
- [ ] Benchmark result is plausible (no suspiciously fast iterations indicating context reuse)

## Dependencies

- None — runs on the current schema before any other WP changes
