---
verblock: "11 Apr 2026:v0.1: vscode - Initial version"
wp_id: WP-18
title: "Enable optional log viewing in cargo tests"
scope: Small
status: Done
---

# WP-18: Enable optional log viewing in cargo tests

## Objective

Make it easy for developers to opt in to seeing tracing/log output when running cargo tests, without requiring output on every run.

## Deliverables

- `logging::init_test_tracing()` added to `logging.rs` using `with_test_writer()` so output respects `--nocapture`
- `init_test_tracing()` wired into `server_integration_tests.rs` via `init_server()`
- `init_test_tracing()` wired into `entry_service_integration_tests.rs` via `init_entry_service()`
- `init_test_tracing()` wired into `index_service_integration_tests.rs` via `init_index_service()`

## Acceptance Criteria

- [ ] `init_tracing()` helper available in test context using `with_test_writer()`
- [ ] Running `RUST_LOG=debug cargo test -- --nocapture` shows log output for tests that call the helper
- [ ] No log output appears in normal `cargo test` runs

## Dependencies

- WP-01 (tracing-subscriber init already added)
