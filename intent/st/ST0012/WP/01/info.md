---
verblock: "15 May 2026:v0.1: vscode - Initial version"
wp_id: WP-01
title: "Add apply_migrations config flag to DatastoreConfig"
scope: Small
status: Done
---

# WP-01: Add apply_migrations config flag to DatastoreConfig

## Objective

Add an `apply_migrations` boolean field to `DatastoreConfig` that controls whether the server is permitted to run database migrations on startup. Default is `false` so that existing deployments are unaffected and migration application is an explicit opt-in.

## Deliverables

- `apply_migrations: bool` field added to `DatastoreConfig` with `#[serde(default)]` defaulting to `false`
- Config parsing tests updated / added to confirm the flag round-trips correctly and defaults to `false` when absent

## Acceptance Criteria

- [ ] `apply_migrations` absent from config file → resolves to `false`
- [ ] `apply_migrations: true` in config file → resolves to `true`
- [ ] Existing config fixtures continue to parse without error

## Dependencies

- None
