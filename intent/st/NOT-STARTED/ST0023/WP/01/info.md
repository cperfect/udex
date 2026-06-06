---
verblock: "06 Jun 2026:v0.1: vscode - Initial version"
wp_id: WP-01
title: "Adopt serde-saphyr and retire serde_yaml"
scope: Small
status: Not Started
---

# WP-01: Adopt serde-saphyr and retire serde_yaml

## Objective

Make `serde-saphyr` the single YAML library for the project, retiring the archived `serde_yaml` (0.9). This WP is independent of the config format change and ships on its own: it migrates the existing `-o yaml` CLI output to `serde-saphyr` and removes `serde_yaml`. It is the prerequisite for WP-02 (config parsing).

## Deliverables

- `serde-saphyr` added to the workspace `Cargo.toml`, pinned to a specific minor version (pre-1.0).
- `-o yaml` CLI output migrated from `serde_yaml::to_string` to `serde_saphyr::to_string` in `projects/rust/cli/src/commands/index.rs`, `entry.rs`, `token.rs`, using `SerializerOptions` tuned for clean, idiomatic output (unquoted keys, normal block layout).
- `serde_yaml` removed from the workspace `Cargo.toml:44` and from `cli`, `datastore`, `server` `Cargo.toml`.

## Acceptance Criteria

- [ ] `serde_yaml` no longer appears anywhere in `Cargo.toml`/`Cargo.lock`.
- [ ] `udex index get … -o yaml` (and entry/token equivalents) produce clean, idiomatic YAML (unquoted keys, no odd scalar/sequence wrapping) — verified against snapshot or example output.
- [ ] `cargo fmt --check`, `cargo clippy`, `cargo test` all pass.

## Dependencies

- None (foundation WP). Blocks WP-02.
