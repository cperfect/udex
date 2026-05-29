# Implementation - ST0022: JWKS refresh

## As-Built Summary

All five work packages complete. The implementation lives entirely in
`projects/rust/server/src/authz.rs` with integration tests in
`projects/rust/server/tests/jwks_refresh_tests.rs`.

## Data Model

`KeySource::Jwks` was refactored from a bare `HashMap<String, (DecodingKey, Algorithm)>`
to a `JwksCache` struct:

```text
JwksCache {
    url: String
    client: reqwest::Client                              (reused across refreshes)
    keys: tokio::sync::RwLock<HashMap<kid, (DecodingKey, Algorithm)>>
    refresh_ctrl: tokio::sync::Mutex<RefreshCtrl>
        consecutive_failures: u32
        backoff_until: Option<Instant>
    max_failed_refreshes: u32                            (default 5)
    backoff_factor_secs: u64                             (default 3)
    max_age_secs: u64                                    (default 86400; 0 = disabled)
    expiry_task: OnceLock<tokio::task::AbortHandle>
}
```

`parse_jwks` is extracted as a module-level free function (Highlander: single
source for building the key map from a `JwkSet`).

## Refresh Triggers

### Cache-miss (WP02)

`decoding_key_for` is async. Fast path: read lock, clone entry, release lock.
On miss: `try_refresh(Some(kid)).await` acquires `Mutex<RefreshCtrl>`,
double-checks the map (another task may have refreshed first), enforces DoS
controls, then calls `fetch_and_parse_jwks`. On success: atomically replaces
the key map and resets `RefreshCtrl`. On failure: increments
`consecutive_failures` and sets `backoff_until`.

### Configured expiry (WP03)

`run_expiry_loop(Weak<AuthzInterceptorInner>)` is a free async function
spawned in `AuthzInterceptor::new` (iff `max_age_secs > 0`). It releases the
`Arc` before sleeping so server shutdown is not blocked. After sleeping it
calls `try_refresh(None)` through the same shared `Mutex<RefreshCtrl>`.
`JwksCache::drop` calls `expiry_task.abort_handle().abort()` via a `Drop`
impl. The `AbortHandle` is stored via `OnceLock::set` immediately after the
`Arc` is constructed — the only window where the handle can be set once and
never changed.

## DoS Controls

Both triggers share `Mutex<RefreshCtrl>`. The gate check (`consecutive_failures
>= max_failed_refreshes`) runs before the backoff check, so once the gate
fires no further fetches are attempted until restart regardless of backoff
state.

`compute_backoff(consecutive_failures, factor_secs)` uses equal-jitter
exponential backoff: `temp = min(300, factor^consecutive_failures)`,
`delay = temp/2 + rand(0..temp/2)`.

`compute_expiry_deadline(max_age_secs)` applies ±1% jitter:
`jitter_range = max_age / 100`, result in `[0.99·max_age, 1.01·max_age]`.

## Key Technical Decisions

**Owned `(DecodingKey, Algorithm)` from lookup** — the map lives behind a
`tokio::sync::RwLock`; a guard cannot outlive an `await` point. Cloning on
lookup is cheap relative to network I/O and removes the lifetime coupling.

**`OnceLock<AbortHandle>` lifecycle** — the expiry task must not prevent
server shutdown. The task closure holds `Weak<AuthzInterceptorInner>`; when
the last strong `Arc` drops, `upgrade()` returns `None` and the loop exits.
`Drop` calls `abort()` as a safety net for in-progress sleeps.

**`std::sync::OnceLock` (not `tokio`)** — the handle is set synchronously
once after `Arc::new`; there is no async contention, so the standard-library
`OnceLock` is the right tool.

## Dependencies Added

- `rand = "0.8"` — workspace dependency, used for backoff jitter and expiry jitter
- `base64 = "0.22"` — server dev dependency, used in integration tests to
  decode Hydra's JWK private key `d` field for SEC1 DER construction

## Integration Test Approach (WP04)

Tests use dedicated Hydra custom key sets (`/admin/keys/udex-jwks-refresh-{uuid}`)
via `ory_hydra_client::apis::jwk_api`. Each test server's `jwks_url` points at
its own isolated key set — completely independent of `/.well-known/jwks.json`
used by the rest of the test suite. Private keys are extracted from Hydra's
admin API response (`d` field, base64url) and converted to `EncodingKey` via a
minimal SEC1 ECPrivateKey DER construction (RFC 5915 §3, P-256, 51 bytes).

## Deviations from Design

None significant. One minor adjustment: `compute_backoff` uses
`factor^consecutive_failures` (1-indexed after increment) rather than the
design's 0-indexed `factor^(consecutive_failures - 1)`. This gives a
slightly more useful 1–2 s delay on the first failure rather than 0 s, which
better protects against immediate re-hammering.
