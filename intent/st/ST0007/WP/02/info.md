---
verblock: "27 Apr 2026:v0.1: vscode - Initial version"
wp_id: WP-02
title: "Update AuthNzConfig validation: accept jwks_url as alternative to jwt_public_key_path"
scope: Small
status: Not Started
---

# WP-02: Update AuthNzConfig validation: accept jwks_url as alternative to jwt_public_key_path

## Objective

Update `AuthNzConfig::validate()` in `udex_server::config` so that it accepts
`jwks_url` as a valid alternative to `jwt_public_key_path`. Exactly one of the
two must be set; providing both or neither is a `ConfigValidation` error.

## Deliverables

- `AuthNzConfig::validate()` rewritten to enforce the mutual-exclusion rule.
- `AuthNzConfig` struct updated: `jwks_url` field already exists; ensure it is
  included in the `Default` impl (as `None`).
- Existing unit tests in `config.rs` updated; new tests added for JWKS-only and
  both-set / neither-set error cases.

## Acceptance Criteria

- [ ] `validate()` returns `Ok(())` when only `jwt_public_key_path` is set
      (existing behaviour preserved).
- [ ] `validate()` returns `Ok(())` when only `jwks_url` is set (new).
- [ ] `validate()` returns `ConfigValidation` error when both are set.
- [ ] `validate()` returns `ConfigValidation` error when neither is set.
- [ ] `jwt_issuer` and `jwt_audience` remain required in all cases.
- [ ] All existing config unit tests pass.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and
      `cargo test -p udex-server` pass.

## Dependencies

Can be done in parallel with WP-01; no ordering dependency.
