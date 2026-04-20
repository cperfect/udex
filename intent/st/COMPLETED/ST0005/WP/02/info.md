---
verblock: "20 Apr 2026:v0.1: vscode - Initial version"
wp_id: WP-02
title: "Implement xxh3_context_hash in udex-api"
scope: Small
status: Done
---

# WP-02: Implement xxh3_context_hash in udex-api

## Objective

Replace the SHA-1 hashing implementation in `udex-api` with xxh3, removing the `sha1` dependency entirely.

## Deliverables

- `projects/rust/Cargo.toml` — add `xxhash-rust` workspace dependency (with `xxh3` feature); remove `sha1`
- `projects/rust/api/Cargo.toml` — swap dependency accordingly
- `projects/rust/api/src/hash.rs` — replace `sha1_context_hash` with `xxh3_context_hash`; delete the SHA-1 implementation
- Unit tests in `hash.rs` updated to use `xxh3_context_hash`

## Acceptance Criteria

- [x] `xxh3_context_hash` exists and is exported from `udex-api::hash`
- [x] `sha1_context_hash` is deleted
- [x] `sha1` crate is removed from the dependency tree
- [x] All hash unit tests (determinism, ordering, collision) pass against the new implementation
- [x] `cargo test -p udex-api` passes

## Dependencies

- WP-01 (Xxh3 proto variant must exist)
