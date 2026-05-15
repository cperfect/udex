---
verblock: "15 May 2026:v0.1: vscode - Initial version"
wp_id: WP-03
title: "Add migrate check and migrate apply CLI commands"
scope: Small
status: Done
---

# WP-03: Add migrate check and migrate apply CLI commands

## Objective

Add `migrate check` and `migrate apply` subcommands to the CLI so operators can inspect and advance the database schema independently of the server process.

## Deliverables

- `udex migrate check` — connects to the database, prints current version and latest available version, exits 0 if up-to-date or non-zero if behind
- `udex migrate apply` — connects to the database and applies all outstanding migrations, then runs `check_migration_version()` to confirm success
- Both commands accept the same datastore config flags as the server (connection URL, TLS, etc.)
- Tests covering both commands against the integration test database

## Acceptance Criteria

- [ ] `migrate check` exits 0 and prints current == latest when up-to-date
- [ ] `migrate check` exits non-zero and prints a clear message when behind
- [ ] `migrate apply` advances the schema and exits 0 on success
- [ ] `migrate apply` reports an error and exits non-zero on failure

## Dependencies

- WP-01 (apply_migrations config flag)
