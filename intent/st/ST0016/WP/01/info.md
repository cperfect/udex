---
verblock: "16 May 2026:v0.1: vscode - Initial version"
wp_id: WP-01
title: "Update proto schema: add display_name and document name format"
scope: Small
status: Not Started
---

# WP-01: Update proto schema: add display_name and document name format

## Objective

Update `projects/protobuf/udex.index.v1.proto` to add a `display_name` field to all relevant messages, update the `name` field comment to document the allowed character set, and make `display_name` and `description` explicitly mandatory in `CreateIndexRequest`. Regenerate the Rust types.

## Deliverables

- `projects/protobuf/udex.index.v1.proto`:
  - Add `string display_name` to `Index` (tag 12, after `updated_by`).
  - Add `string display_name` to `CreateIndexRequest` (tag 8) — mandatory (no `optional`).
  - Add `optional string display_name` to `IndexUpdate` (tag 7) — mutable.
  - Update `name` comment everywhere it appears: `// Identifier: Unicode letters, digits, hyphens, underscores. Immutable, unique.`
  - Update `description` comment in `CreateIndexRequest` to remove "optional" — it is now mandatory.

- `projects/rust/api/src/generated/udex.index.v1.rs` — regenerate from the updated proto (run `cargo build` in the `api` crate which triggers `build.rs`).

## Acceptance Criteria

- [ ] `display_name` field present in `Index`, `CreateIndexRequest`, and `IndexUpdate` with correct tags
- [ ] `name` field comment in proto accurately describes the allowed character set
- [ ] `description` in `CreateIndexRequest` is no longer commented as "optional"
- [ ] Generated Rust file updated to match the proto changes
- [ ] `cargo fmt --check`, `cargo clippy --all-targets`, `cargo build` all pass

## Dependencies

- None
