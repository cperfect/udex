---
verblock: "15 May 2026:v0.1: vscode - Initial version"
wp_id: WP-02
title: "Enforce check_migration_version on every startup"
scope: Small
status: Not Started
---

# WP-02: Enforce check_migration_version on every startup

## Objective

Change `server::start()` to conditionally run `migrate()` based on `apply_migrations`, then always call `check_migration_version()` afterwards. If the version check fails the server must log a clear error and exit rather than starting against a mismatched schema.

## Deliverables

- `server::start()` updated: run `migrate()` only when `datastore_config.apply_migrations == true`
- `check_migration_version()` called unconditionally after the conditional migrate step
- Structured log message at ERROR level on version mismatch, including current and expected versions
- Integration tests: server starts when DB is current; server fails with appropriate error when DB is behind and `apply_migrations = false`; server migrates and starts when DB is behind and `apply_migrations = true`

## Acceptance Criteria

- [ ] Server starts normally when DB is at the latest version regardless of `apply_migrations`
- [ ] Server refuses to start when DB is behind and `apply_migrations = false`, with a logged error including version numbers
- [ ] Server migrates and starts when DB is behind and `apply_migrations = true`

## Dependencies

- WP-01 (apply_migrations config flag)
