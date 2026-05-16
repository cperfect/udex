---
verblock: "16 May 2026:v0.1: vscode - Initial version"
wp_id: WP-02
title: "Datastore: add display_name column and update read/write paths"
scope: Small
status: Done
---

# WP-02: Datastore: add display_name column and update read/write paths

## Objective

Add a `display_name TEXT NOT NULL` column to the `"index"` table in the existing migration and update the PostgreSQL read/write paths in `udex-datastore` to store and retrieve it.

## Deliverables

- `datastore/migrations/postgres/01_initial_schema.sql` — add `display_name TEXT NOT NULL` column to the `"index"` table (after `description`).

- `datastore/src/postgres.rs` (or wherever the index CRUD lives) — update every SQL INSERT, SELECT, and UPDATE that touches the `"index"` table to include `display_name`:
  - `create_index`: bind `index.display_name` in the INSERT.
  - `get_index` / `list_indices`: read `display_name` from the row and populate `Index.display_name`.
  - `update_index`: accept and apply `IndexUpdate.display_name` when present.

- `datastore/README.md` — add `display_name` row to the `"index"` table documentation, noting it is a short free-text label for UI use.

- `datastore/tests/postgres_integration_tests.rs` — update any helper that builds an `Index` for test fixtures to supply a `display_name`; add an assertion in the create/describe path that `display_name` round-trips correctly.

## Acceptance Criteria

- [x] Migration file includes `display_name TEXT NOT NULL` in the `"index"` table
- [x] `create_index` persists `display_name` to the DB
- [x] `get_index` and `list_indices` return the correct `display_name`
- [x] `update_index` applies `display_name` changes when provided
- [x] README Data Model section documents `display_name`
- [x] Integration tests confirm `display_name` round-trips
- [x] `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test` all pass

## Dependencies

- WP-01 must be complete so the generated `Index` and `IndexUpdate` types include `display_name`
