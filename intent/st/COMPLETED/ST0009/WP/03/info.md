---
verblock: "06 May 2026:v0.1: vscode - Initial version"
wp_id: WP-03
title: "Replace migrations with entry_context table"
scope: Small
status: Done
---

# WP-03: Replace migrations with entry_context table

## Objective

Discard the existing `entry` + `context` migrations and replace them with a single migration that creates the `entry_context` table. The new table collapses both concerns: a server-generated UUID primary key, an inline context record, and a `UNIQUE` constraint on `context_hash` that enforces the 1:1 invariant at the database level.

## Deliverables

- Old `entry` and `context` migration files deleted (or replaced)
- New migration file creating `entry_context` with columns: `key UUID PRIMARY KEY`, `index_name TEXT NOT NULL REFERENCES index(name)`, `context_hash TEXT NOT NULL UNIQUE`, `pairs JSONB NOT NULL`, `dek TEXT`, `kek_id TEXT`, `hash_algorithm TEXT NOT NULL`
- Index on `(index_name, context_hash)` for index-scoped lookups

## Acceptance Criteria

- [ ] `sqlx database reset -f --source migrations/postgres` applies cleanly against a fresh database
- [ ] `entry_context` table exists with correct schema and constraints
- [ ] `UNIQUE(context_hash)` constraint is present and enforced
- [ ] Old `entry` and `context` tables are gone
- [ ] No data migration logic needed (confirmed: no live installs)

## Dependencies

- WP-02: Proto definitions should be settled before schema is finalised (column set is driven by context fields)
