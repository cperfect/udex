---
verblock: "06 Apr 2026:v0.1: vscode - Initial version"
wp_id: WP-09
title: "Move tracing-test to workspace dependencies"
scope: Small
status: Not Started
priority: major
---

# WP-09: Move tracing-test to workspace dependencies

## Review Finding

🟠 **Major** — `tracing-test = "0.2"` is duplicated directly in both `api/Cargo.toml` and `server/Cargo.toml` dev-dependencies rather than being centralised in the workspace. This violates the project's established pattern and risks version drift.

## Objective

Centralise `tracing-test` in the workspace `Cargo.toml` and reference it from both crates via `{ workspace = true }`.

## Fix

In `projects/rust/Cargo.toml` `[workspace.dependencies]`:
```toml
tracing-test = "0.2"
```

In both `api/Cargo.toml` and `server/Cargo.toml` dev-dependencies:
```toml
tracing-test = { workspace = true }
```

## Acceptance Criteria

- [ ] `tracing-test` is declared in `[workspace.dependencies]`
- [ ] Both `api/Cargo.toml` and `server/Cargo.toml` reference it via `{ workspace = true }`
- [ ] All logging tests still pass

## Dependencies

- None
