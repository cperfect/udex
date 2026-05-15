---
verblock: "15 May 2026:v0.1: vscode - Initial version"
wp_id: WP-02
title: "Datastore trait and PostgreSQL implementation"
scope: Small
status: Done
---

# WP-02: Datastore trait and PostgreSQL implementation

## Objective

Add a `lookup_or_create_entry` method to the `Datastore` trait and implement it in `PostgresDatastore`. The method finds an entry by context hash and — if none exists — creates one, returning both the key and a `created` boolean.

## Deliverables

- `projects/rust/datastore/src/lib.rs`: new method `lookup_or_create_entry(entry: Entry) -> Result<(Uuid, bool), Error>` on the `Datastore` trait, where `bool` is `true` if the entry was created, `false` if it already existed.
- `projects/rust/datastore/src/postgres.rs`: implementation using an upsert (INSERT ... ON CONFLICT DO NOTHING) combined with a SELECT to determine whether a row was inserted.
- Integration tests covering: entry created (bool=true) and entry already exists (bool=false, same key returned).

## Acceptance Criteria

- [ ] `cargo build -p udex-datastore` succeeds.
- [ ] Integration tests for both created and found paths pass against PostgreSQL.
- [ ] The existing `create_entry` method is unchanged (no signature breakage).

## Dependencies

- WP-01 must be complete (API crate must compile for datastore to reference `Context` type).
