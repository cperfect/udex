---
verblock: "12 May 2026:v0.1: vscode - Initial version"
wp_id: WP-02
title: "Datastore trait and PostgreSQL implementation"
scope: Small
status: Done
---

# WP-02: Datastore trait and PostgreSQL implementation

## Objective

Add `delete_index` to the `Datastore` trait and implement it in the PostgreSQL backend. The operation must fail with a specific error if the index still has entries.

## Deliverables

- `projects/rust/datastore/src/lib.rs`: add `async fn delete_index(&self, name: &str) -> Result<(), Error>` to the `Datastore` trait; add a new `Error::IndexNotEmpty` variant
- `projects/rust/datastore/src/postgres.rs`: implement `delete_index` — check `entry_context` for rows with `index_name = name`; return `Error::IndexNotEmpty` if any exist; otherwise `DELETE FROM "index" WHERE name = $1`; return `Error::InvalidIndex` if the row was not found
- `projects/rust/datastore/tests/postgres_integration_tests.rs`: integration tests covering delete of empty index (ok), delete of non-empty index (IndexNotEmpty), delete of non-existent index (InvalidIndex)

## Acceptance Criteria

- [x] `Datastore::delete_index` is in the trait
- [x] `Error::IndexNotEmpty` variant exists
- [x] PostgreSQL implementation deletes only when no entries exist
- [x] Integration tests cover: delete empty index (ok), delete non-empty index (IndexNotEmpty), delete non-existent index (InvalidIndex)
- [x] `cargo test -p udex-datastore` passes (including new integration tests)

## Dependencies

- WP-01 (trait must be in place before server can use it, but datastore WP is independent of proto)
