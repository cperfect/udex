---
verblock: "06 Jun 2026:v0.1: vscode - Initial version"
wp_id: WP-02
title: "Convert config parsing from TOML to YAML"
scope: Small
status: Not Started
---

# WP-02: Convert config parsing from TOML to YAML

## Objective

Replace the TOML config format with YAML in the CLI config loader — a replacement, not an addition. After this WP, Udex reads `udex.yaml` via `serde-saphyr`, and the `toml` crate is gone.

## Deliverables

- `toml::from_str` → `serde_saphyr::from_str` at both load seams: `UdexConfig::load` (`config.rs:115`) and the datastore-only `Wrapper` load (`config.rs:276`). No config struct/field changes.
- Default config path renamed `udex.toml` → `udex.yaml` and the `UDEX_CONFIG` env default updated (`cli.rs:97,115,123`).
- Config round-trip/parse tests (`config.rs:473–543`) ported from `toml` to `serde_saphyr` (drop `toml::to_string_pretty`/`toml::from_str`).
- `toml` dependency removed from `projects/rust/cli/Cargo.toml:26` and workspace `Cargo.toml:58`.

## Acceptance Criteria

- [ ] A valid `udex.yaml` loads, validates, and binds all `secrets-rs` URNs (file + env) exactly as the old TOML did — URNs unchanged.
- [ ] `toml` no longer appears anywhere in `Cargo.toml`/`Cargo.lock`.
- [ ] `cargo fmt --check`, `cargo clippy`, `cargo test` all pass.

## Dependencies

- **WP-01** (serde-saphyr adopted). Blocks WP-03 and WP-04.
