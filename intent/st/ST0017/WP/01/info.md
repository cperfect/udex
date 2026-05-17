---
verblock: "17 May 2026:v0.1: vscode - Initial version"
wp_id: WP-01
title: "Create udex-test-utils workspace crate with shared fixtures"
scope: Small
status: Not Started
---

# WP-01: Create udex-test-utils workspace crate with shared fixtures

## Objective

Create a new `udex-test-utils` workspace crate that consolidates shared test fixture code currently copy-pasted across `server`, `sdk`, and `cli`. Used only as a dev-dependency — never compiled into production binaries.

## Deliverables

- `projects/rust/test-utils/Cargo.toml` — new crate; dev-only deps on `udex-server`, `udex-datastore` (with `integration_test` feature), `secrets-rs`, `dotenvy`, `ory-hydra-client`, `oauth2`, `tonic`, `tokio`.
- `projects/rust/test-utils/src/lib.rs` — exports:
  - `bind_file_secret(path: &str) -> Secret<String>` — identical across 5 files today
  - `hydra_public_url() -> String` and `hydra_admin_url() -> String` — `dotenvy` + env var with fallback
  - `register_hydra_client(admin_url, client_id, client_secret, scopes) -> ()` — Hydra OAuth2 client upsert
  - `acquire_oauth2_token(token_url, client_id, client_secret, scopes) -> String` — client_credentials token fetch
- `projects/rust/Cargo.toml` — add `test-utils` to `[workspace] members`.
- `server/Cargo.toml`, `sdk/Cargo.toml`, `cli/Cargo.toml` — add `udex-test-utils` as a `[dev-dependencies]` entry.
- `intent/llm/MODULES.md` — register the new crate and its exported concerns.

## Acceptance Criteria

- [ ] `udex-test-utils` builds cleanly with `cargo build -p udex-test-utils`
- [ ] All exported helpers compile and pass `cargo clippy`
- [ ] `cargo fmt --check` passes
- [ ] No production binary includes `udex-test-utils` (confirmed by `cargo tree --no-dev -p udex-server` etc.)

## Dependencies

- None — this WP creates the foundation; WP-02 and WP-03 consume it
