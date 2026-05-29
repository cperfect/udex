# Tasks - ST0022: JWKS refresh

## Work Packages

| WP | Title                          | Scope  | Status      |
| -- | ------------------------------ | ------ | ----------- |
| 01 | Scaffold: config + JwksCache   | Medium | Not Started |
| 02 | Cache-miss refresh             | Medium | Not Started |
| 03 | Expiry background task         | Small  | Not Started |
| 04 | Integration tests              | Medium | Not Started |
| 05 | Docs                           | Small  | Not Started |

## WP01 — Scaffold: config + JwksCache struct

- [ ] Add `jwks_max_failed_refreshes`, `jwks_backoff_factor_secs`, `jwks_max_age_secs` to `AuthzConfig` with defaults and validation
- [ ] Define `JwksCache` and `RefreshCtrl` structs
- [ ] Migrate `KeySource::Jwks` from bare `HashMap` to `JwksCache`
- [ ] Move startup fetch into `JwksCache` construction; retain `client` + `url`
- [ ] Make `decoding_key_for` and `validate_jwt` `async`; return owned `(DecodingKey, Algorithm)`
- [ ] Add `rand` as a dependency to `server/Cargo.toml`
- [ ] All existing tests pass

## WP02 — Cache-miss refresh

- [ ] Implement `JwksCache::fetch_and_parse_jwks` (shared HTTP fetch + parse + key-map build)
- [ ] Implement `JwksCache::try_refresh` with double-checked locking and DoS controls
- [ ] Implement `compute_backoff` (equal-jitter exponential, cap 300 s)
- [ ] Wire miss path in `decoding_key_for`
- [ ] Unit tests: gate fires at max attempts; backoff suppresses retries; success resets state; concurrent misses produce one fetch

## WP03 — Expiry background task

- [ ] Implement `compute_expiry_deadline` with ±1% jitter
- [ ] Implement `run_expiry_loop` (`sleep → Weak::upgrade check → try_refresh`)
- [ ] Spawn task via `OnceLock<AbortHandle>` after `Arc<AuthzInterceptorInner>` is constructed
- [ ] Implement `Drop` for `JwksCache` to abort the task
- [ ] Unit tests: jitter within ±1% bounds; task exits when `Weak` is dead

## WP04 — Integration tests

- [ ] New file `projects/rust/server/tests/jwks_refresh_tests.rs`
- [ ] Use `ory_hydra_client::apis::jwk_api` to add/rotate keys in Hydra's key set
- [ ] Test: key rotation → cache-miss → refresh → request with new kid succeeds
- [ ] Test: JWKS endpoint down → cache retained, errors logged, existing keys served
- [ ] Test: max failed attempts gate fires; server continues serving with stale cache

## WP05 — Docs

- [ ] `projects/rust/server/README.md`: rewrite Authorization section to describe both refresh mechanisms and the three new config fields
- [ ] `README.md` (root): update Authorization table row if needed
- [ ] `intent/st/ST0022/impl.md`: as-built notes covering all WPs
- [ ] `intent/st/ST0022/tasks.md`: mark WPs complete

## Dependencies

WP02 depends on WP01. WP03 depends on WP01. WP04 depends on WP02 and WP03. WP05 depends on all preceding WPs.
