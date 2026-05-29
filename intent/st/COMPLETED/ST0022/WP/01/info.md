---
verblock: "29 May 2026:v0.1: vscode - Initial version"
wp_id: WP-01
title: "Scaffold: config + JwksCache struct"
scope: Medium
status: Done
---

# WP-01: Scaffold: config + JwksCache struct

## Objective

Refactor the JWKS path to introduce `JwksCache` and `RefreshCtrl` structs,
make key lookup async, and scaffold all fields required by WP02 and WP03
without changing any observable behaviour. All existing tests must remain
green throughout.

## Deliverables

- `parse_jwks` module-level helper — single source of truth for building
  the `kid → (DecodingKey, Algorithm)` map from a `JwkSet`
- `RefreshCtrl` struct with `consecutive_failures` and `backoff_until` fields
- `JwksCache` struct replacing `KeySource::Jwks(HashMap<...>)`, holding
  `url`, `client`, `keys: RwLock<HashMap<...>>`, `refresh_ctrl: Mutex<RefreshCtrl>`,
  `max_failed_refreshes`, `backoff_factor_secs`, `max_age_secs`,
  and `expiry_task: OnceLock<AbortHandle>`
- `Drop` impl on `JwksCache` that aborts the expiry task handle if set
- `decoding_key_for` and `validate_jwt` made `async`; return owned
  `(DecodingKey, Algorithm)` (no lifetime coupling with the `RwLock` guard)
- Three new `AuthzConfig` fields: `jwks_max_failed_refreshes`,
  `jwks_backoff_factor_secs`, `jwks_max_age_secs` — all `Option` with
  sane defaults applied at `JwksCache` construction
- `rand = "0.8"` added to workspace and server `Cargo.toml`
- All `AuthzConfig` struct literals updated across the workspace

## Acceptance Criteria

- [x] All 41 server unit tests pass
- [x] `cargo clippy --all-targets -- -D warnings` clean across the workspace
- [x] No behaviour change — startup JWKS fetch and static-key paths work identically

## Dependencies

None — first work package.
