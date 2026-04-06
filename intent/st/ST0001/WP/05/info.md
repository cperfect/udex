---
verblock: "06 Apr 2026:v0.1: vscode - Initial version"
wp_id: WP-05
title: "init_tracing is never called - connect to binary entry point"
scope: Small
status: Not Started
priority: critical
---

# WP-05: init_tracing is never called — connect to binary entry point

## Review Finding

🔴 **Critical** — `init_tracing()` is never called. The design doc (`ST0001/design.md`) explicitly states it "must be called at binary startup before `serve()`", but there is no `main.rs` or equivalent that does so. Without calling `init_tracing()`, the global subscriber is never set, meaning all `tracing::info!`, `tracing::warn!`, and `tracing::error!` calls throughout the codebase are silently discarded at runtime.

## Objective

Ensure `init_tracing()` is called before `serve()` so that structured logging is actually active at runtime.

## Deliverables

- Call `udex_server::logging::init_tracing()` at the binary entry point before `serve()`. Since the CLI crate is not yet implemented, call it at the top of `serve()` in `server.rs` as the interim solution, guarded against double-initialisation (see WP-06).

## Acceptance Criteria

- [ ] `init_tracing()` is invoked before any tracing macros can fire
- [ ] Starting the server produces JSON log output
- [ ] Double-initialisation is safe (depends on WP-06)

## Dependencies

- WP-06 (guard against double-init) should be implemented first or together
