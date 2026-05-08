---
verblock: "08 May 2026:v0.1: vscode - Initial version"
wp_id: WP-01
title: "Crate scaffold and workspace integration"
scope: Small
status: Done
---

# WP-01: Crate scaffold and workspace integration

## Objective

Create the `udex-sdk` crate under `sdk/` and integrate it into the Rust workspace so it compiles cleanly and is visible to other crates.

## Deliverables

- `sdk/` directory with `Cargo.toml` declaring crate `udex-sdk`
- Workspace `Cargo.toml` updated to include `sdk/` member
- `sdk/src/lib.rs` with crate-level rustdoc overview (stub)
- `MODULES.md` updated with the new crate

## Acceptance Criteria

- [ ] `cargo build -p udex-sdk` succeeds from workspace root
- [ ] `cargo clippy -p udex-sdk` produces no warnings
- [ ] `MODULES.md` reflects the new crate

## Dependencies

- None
