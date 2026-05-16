---
verblock: "16 May 2026:v0.1: vscode - Initial version"
wp_id: WP-03
title: "Server validation: name format, mandatory display_name and description"
scope: Small
status: Not Started
---

# WP-03: Server validation: name format, mandatory display_name and description

## Objective

Add server-layer validation to enforce the index `name` character-set constraint (Unicode letters, digits, hyphens, underscores) and verify that both `display_name` and `description` are non-empty on `CreateIndexRequest`. Update the init path to propagate `display_name`.

## Deliverables

- `server/src/index.rs` — `create_index` handler:
  - Validate `req.name` against the allowed pattern (Unicode letters `\p{L}`, digits `\p{N}`, hyphens `-`, underscores `_`). Return `Status::invalid_argument` on failure with a message naming the constraint.
  - Validate `req.display_name` is non-empty. Return `Status::invalid_argument` if blank.
  - Validate `req.description` is non-empty. Return `Status::invalid_argument` if blank.
  - Populate `Index.display_name` from `req.display_name` when building the `Index` to persist.

- `server/src/index.rs` — `init` path: populate `display_name` on the synthesised `Index` from `update.display_name` (with a mandatory check matching the handler).

- `server/src/index.rs` — `update_index` handler: pass `display_name` through when present in `IndexUpdate` (no new validation needed beyond non-empty if set).

- `server/tests/` (or integration test suite) — add cases:
  - Name with invalid characters → `invalid_argument`.
  - Empty `display_name` → `invalid_argument`.
  - Empty `description` → `invalid_argument`.
  - Valid name, display_name, description → success.

## Acceptance Criteria

- [ ] `name` containing characters outside the allowed set is rejected with `invalid_argument`
- [ ] Empty or whitespace-only `display_name` is rejected with `invalid_argument`
- [ ] Empty or whitespace-only `description` is rejected with `invalid_argument`
- [ ] Valid requests succeed end-to-end (create → describe round-trip confirms `display_name`)
- [ ] `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test` all pass

## Dependencies

- WP-01 (proto schema with `display_name`) and WP-02 (datastore reads/writes `display_name`) must be complete
