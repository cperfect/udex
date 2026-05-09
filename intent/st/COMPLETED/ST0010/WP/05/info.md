---
verblock: "08 May 2026:v0.1: vscode - Initial version"
wp_id: WP-05
title: "Integration tests"
scope: Small
status: Done
---

# WP-05: Integration tests

## Objective

Provide integration tests that exercise the full SDK against the compose stack (server + Hydra + Postgres) to give confidence the SDK works end-to-end.

## Deliverables

- `sdk/tests/integration_tests.rs` covering: connect, authenticate, create index, create/get/delete entry, bulk ops
- Tests run in CI against the compose environment (same pattern as `server/tests/`)
- At least one negative test: invalid credentials return `Error::Auth`

## Acceptance Criteria

- [ ] All integration tests pass against the compose stack
- [ ] `cargo test -p udex-sdk` runs the integration tests (with `DATABASE_URL` / compose env)
- [ ] No flaky tests — each test cleans up its own state

## Dependencies

- WP-04
