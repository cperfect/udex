---
verblock: "15 May 2026:v0.1: vscode - Initial version"
intent_version: 2.4.0
status: WIP
slug: datastore-migration-control-and-validation
created: 20260515
completed:
---

# ST0012: Datastore migration control and validation

## Objective

Make database migration execution configurable (default off) and enforce that the database schema is always validated against the current code version on startup, failing fast with a clear error if there is a mismatch. Add CLI commands to check and apply migrations explicitly. Update documentation to reflect the new behaviour.

## Context

Currently the server unconditionally runs `migrate()` on every startup (server.rs:30), which applies all outstanding migrations automatically. This is unsafe in production because:

1. **Unintended schema changes** — operators may not intend to apply migrations on a given deploy and have no way to prevent it.
2. **Code/schema mismatch risk** — if migrations are skipped or partially applied, the server continues running against an incompatible schema with no warning.

The `Migrator` trait already has `check_migration_version()` but it is never called.

### Required changes

- `DatastoreConfig`: add `apply_migrations: bool` (default `false`).
- Server startup: if `apply_migrations` is true, run `migrate()`; then always call `check_migration_version()` and abort with a logged error if versions don't match.
- CLI: add `migrate check` and `migrate apply` commands that operators can run explicitly.
- Docs: update README, FAQ, and any relevant ops/deployment docs.

## Related Steel Threads

- None

## Context for LLM

This document represents a single steel thread - a self-contained unit of work focused on implementing a specific piece of functionality. When working with an LLM on this steel thread, start by sharing this document to provide context about what needs to be done.

### How to update this document

1. Update the status as work progresses
2. Update related documents (design.md, impl.md, etc.) as needed
3. Mark the completion date when finished

The LLM should assist with implementation details and help maintain this document as work progresses.
