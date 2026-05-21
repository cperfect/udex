---
verblock: "21 May 2026:v0.1: Chris Perfect - Initial version"
wp_id: WP-02
title: "Server changes"
scope: Small
status: Not Started
---

# WP-02: Server changes

## Objective

Fix all compilation errors introduced by WP01 and enforce hash algorithm immutability in the server. The `init_indexes` config switches from `UpdateIndexRequest` to `CreateIndexRequest` (the natural home for a required creation-time field). The `init()` function is updated to error at startup if a configured algorithm disagrees with a stored one.

## Deliverables

- `projects/rust/server/src/config.rs` — `init_indexes` field type changed from `Vec<UpdateIndexRequest>` to `Vec<CreateIndexRequest>`
- `projects/rust/server/src/index.rs` — `IndexService::init()` rewritten to accept `Vec<CreateIndexRequest>`:
  - Index does not exist: create it
  - Index exists, algorithm matches: update mutable fields if any differ
  - Index exists, algorithm differs: return startup error
  - `update_index` handler: `hash_algorithm` removed from the empty-field guard
- All integration test fixtures and other call sites updated to compile and pass
- `cargo fmt --check`, `cargo clippy`, `cargo test` all green

## Acceptance Criteria

- [ ] `ServerConfig.init_indexes` is `Vec<CreateIndexRequest>`
- [ ] `IndexService::init()` errors on hash algorithm mismatch for an existing index, with a clear message naming the index, existing algorithm, and configured algorithm
- [ ] `update_index` handler no longer references `hash_algorithm` in its empty-field guard
- [ ] All tests pass (`validate-test-rust.sh`)
- [ ] No clippy warnings (`validate-lint-clippy.sh`)
- [ ] Formatter clean (`validate-lint-fmt.sh`)

## Dependencies

- WP01 must be complete (generated types must compile before server call sites can be fixed).
