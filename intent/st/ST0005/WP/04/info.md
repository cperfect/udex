---
verblock: "20 Apr 2026:v0.1: vscode - Initial version"
wp_id: WP-04
title: "Update tests, benchmarks and docs"
scope: Small
status: Done
---

# WP-04: Update tests, benchmarks and docs

## Objective

Update all integration tests, benchmark fixtures, and documentation to reflect the xxh3 replacement and verify the full test suite passes cleanly.

## Deliverables

- All integration test fixtures updated from `HashAlgorithm::Sha1` to `HashAlgorithm::Xxh3`
- Bench fixtures in `server/benches/` and `datastore/benches/` updated
- `projects/rust/CONTRIBUTING.md` — update any references to SHA-1 or hashing
- `ST0005/impl.md` created documenting the implementation decisions

## Acceptance Criteria

- [x] `cargo test --all` passes with no failures
- [x] `cargo bench --no-run` passes (benchmark compile-check)
- [x] No remaining references to `SHA1`, `Sha1`, or `sha1` anywhere in the workspace (outside of migration comments if any)
- [x] CONTRIBUTING.md reflects the new algorithm

## Dependencies

- WP-01, WP-02, WP-03
