---
verblock: "06 May 2026:v0.1: vscode - Initial version"
wp_id: WP-06
title: "Update benchmarks for unique-context and compare baseline"
scope: Small
status: Done
---

# WP-06: Update benchmarks for unique-context and compare baseline

## Objective

Capture a post-migration Criterion baseline on the new `entry_context` schema using the same updated benchmark code as WP-01, then compare the two baselines. The comparison validates that the schema simplification does not regress throughput and, ideally, shows an improvement from the eliminated join and simplified write path.

## Deliverables

- Criterion baseline captured with `--save-baseline one-table` against the new schema
- Baseline comparison run with `--baseline two-table` (or equivalent `critcmp` report)
- Before/after numbers recorded in `intent/st/NOT-STARTED/ST0009/impl.md`

## Acceptance Criteria

- [ ] `cargo bench --bench entry_service -- --save-baseline one-table` completes without error
- [ ] Comparison report produced (Criterion HTML or `critcmp` output)
- [ ] `create_entry` throughput on new schema is not regressed vs two-table baseline
- [ ] Results documented in impl.md

## Dependencies

- WP-01: `two-table` baseline must exist before comparison is meaningful
- WP-04: New schema and implementation must be in place before capturing `one-table` baseline
