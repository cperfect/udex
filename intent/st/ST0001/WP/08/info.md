---
verblock: "06 Apr 2026:v0.1: vscode - Initial version"
wp_id: WP-08
title: "Move tower-http to workspace dependencies"
scope: Small
status: Done
priority: major
---

# WP-08: Move tower-http to workspace dependencies

## Review Finding

🟠 **Major** — `server/Cargo.toml` declares `tower-http = { version = "0.6", features = ["trace"] }` directly rather than through the workspace. All other dependencies in this project use `{ workspace = true }`. This creates a risk of version drift between crates if tower-http is added to other crates in future.

## Objective

Centralise the `tower-http` version in the workspace `Cargo.toml` and reference it from the server crate via `{ workspace = true }`.

## Fix

In `projects/rust/Cargo.toml` `[workspace.dependencies]`:
```toml
tower-http = { version = "0.6", features = ["trace"] }
```

In `projects/rust/server/Cargo.toml`:
```toml
tower-http = { workspace = true }
```

## Acceptance Criteria

- [ ] `tower-http` is declared in `[workspace.dependencies]`
- [ ] `server/Cargo.toml` references it via `{ workspace = true }`
- [ ] `cargo build` passes

## Dependencies

- None
