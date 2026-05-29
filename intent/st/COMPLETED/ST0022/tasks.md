# Tasks - ST0022: JWKS refresh

## Work Packages

| WP | Title                          | Scope  | Status |
| -- | ------------------------------ | ------ | ------ |
| 01 | Scaffold: config + JwksCache   | Medium | Done   |
| 02 | Cache-miss refresh             | Medium | Done   |
| 03 | Expiry background task         | Small  | Done   |
| 04 | Integration tests              | Medium | Done   |
| 05 | Docs                           | Small  | Done   |

## WP01 — Scaffold: config + JwksCache struct

- [x] Add `jwks_max_failed_refreshes`, `jwks_backoff_factor_secs`, `jwks_max_age_secs` to `AuthzConfig` with defaults and validation
- [x] Define `JwksCache` and `RefreshCtrl` structs
- [x] Migrate `KeySource::Jwks` from bare `HashMap` to `JwksCache`
- [x] Move startup fetch into `JwksCache` construction; retain `client` + `url`
- [x] Make `decoding_key_for` and `validate_jwt` `async`; return owned `(DecodingKey, Algorithm)`
- [x] Add `rand` as a dependency to `server/Cargo.toml`
- [x] All existing tests pass

## WP02 — Cache-miss refresh

- [x] Implement `JwksCache::fetch_and_parse_jwks` (shared HTTP fetch + parse + key-map build)
- [x] Implement `JwksCache::try_refresh` with double-checked locking and DoS controls
- [x] Implement `compute_backoff` (equal-jitter exponential, cap 300 s)
- [x] Wire miss path in `decoding_key_for`
- [x] Unit tests: gate fires at max attempts; backoff suppresses retries; success resets state; concurrent misses produce one fetch

## WP03 — Expiry background task

- [x] Implement `compute_expiry_deadline` with ±1% jitter
- [x] Implement `run_expiry_loop` (`sleep → Weak::upgrade check → try_refresh`)
- [x] Spawn task via `OnceLock<AbortHandle>` after `Arc<AuthzInterceptorInner>` is constructed
- [x] Implement `Drop` for `JwksCache` to abort the task
- [x] Unit tests: jitter within ±1% bounds; task exits when `Weak` is dead

## WP04 — Integration tests

- [x] New file `projects/rust/server/tests/jwks_refresh_tests.rs`
- [x] Use `ory_hydra_client::apis::jwk_api` to add/rotate keys in Hydra's key set
- [x] Test: key rotation → cache-miss → refresh → request with new kid succeeds
- [x] Test: JWKS endpoint down → cache retained, errors logged, existing keys served
- [x] Test: max failed attempts gate fires; server continues serving with stale cache

## WP05 — Docs

- [x] `projects/rust/server/README.md`: rewrite Authorization section to describe both refresh mechanisms and the three new config fields
- [x] `README.md` (root): reviewed — one-liner auth row remains accurate, no change needed
- [x] `intent/st/ST0022/impl.md`: as-built notes covering all WPs
- [x] `intent/st/ST0022/tasks.md`: mark WPs complete

## Dependencies

WP02 depends on WP01. WP03 depends on WP01. WP04 depends on WP02 and WP03. WP05 depends on all preceding WPs.
