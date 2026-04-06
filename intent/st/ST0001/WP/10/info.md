---
verblock: "06 Apr 2026:v0.1: vscode - Initial version"
wp_id: WP-10
title: "Move test-only server deps from [dependencies] to [dev-dependencies]"
scope: Small
status: Not Started
priority: major
---

# WP-10: Move test-only server deps from [dependencies] to [dev-dependencies]

## Review Finding

🟠 **Major** (pre-existing, flagged during review of this changeset) — In `server/Cargo.toml`, `testcontainers`, `testcontainers-modules`, and `tokio-shared-rt` are listed under `[dependencies]` rather than `[dev-dependencies]`. These are test-only crates and should not be compiled into the production binary. This increases binary size and compile times unnecessarily.

## Objective

Move `testcontainers`, `testcontainers-modules`, and `tokio-shared-rt` from `[dependencies]` to `[dev-dependencies]` in `server/Cargo.toml`.

## Acceptance Criteria

- [ ] `testcontainers`, `testcontainers-modules`, and `tokio-shared-rt` are under `[dev-dependencies]` in `server/Cargo.toml`
- [ ] `cargo build` passes (production build does not include test deps)
- [ ] `cargo test` passes (integration tests still work)

## Dependencies

- None — pre-existing issue, safe to fix independently
