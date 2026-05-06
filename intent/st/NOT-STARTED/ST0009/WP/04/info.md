---
verblock: "06 May 2026:v0.1: vscode - Initial version"
wp_id: WP-04
title: "Update datastore trait and PostgreSQL implementation"
scope: Medium
status: Not Started
---

# WP-04: Update datastore trait and PostgreSQL implementation

## Objective

Update the `Datastore` trait and its PostgreSQL implementation to operate against the new `entry_context` table. The write path becomes a single `INSERT … ON CONFLICT (context_hash) DO NOTHING RETURNING key` (idempotent upsert). The read path (`lookup_keys_by_context`) becomes a point lookup returning `Option<Uuid>` instead of `Vec<Uuid>`. The delete path removes by `key` from `entry_context` directly, eliminating the conditional context GC logic.

## Deliverables

- `Datastore` trait updated: `lookup_keys_by_context` returns `Option<Uuid>`; `create_entry` returns existing key on duplicate context
- PostgreSQL implementation queries rewritten for `entry_context`
- Integration tests updated to pass against the new schema and trait signatures
- All SQL formatted per coding guidelines (one parameter/column per line)

## Acceptance Criteria

- [ ] `Datastore::create_entry` called twice with identical context returns the same key both times
- [ ] `Datastore::lookup_keys_by_context` returns `None` when no entry exists, `Some(key)` when one does
- [ ] `cargo test --test postgres_integration_tests` passes against the new schema
- [ ] No references to the old `entry` or `context` tables remain in Rust query strings
- [ ] `cargo clippy` passes with no warnings

## Dependencies

- WP-03: New migration must be applied before integration tests can run
- WP-02: Trait signatures must align with the updated proto return types
