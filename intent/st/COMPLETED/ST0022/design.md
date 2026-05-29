# Design - ST0022: JWKS refresh

## Approach

Two refresh triggers, one shared refresh path:

1. **Cache miss** — when a request presents a `kid` not in the cache, refresh
   inline before returning an error.
2. **Configured expiry** — a background task sleeps until the next expiry
   deadline and then refreshes proactively, even when there is no traffic.

Both triggers call the same underlying fetch-and-update function, serialised
by a shared `Mutex<RefreshCtrl>`. This naturally prevents double-fetching if
both fire simultaneously and allows them to share DoS state (backoff and
failure count).

## Architecture

```text
AuthzInterceptor
└── Arc<AuthzInterceptorInner>
    ├── key_source: KeySource
    │   ├── Static(DecodingKey)
    │   └── Jwks(JwksCache)
    │       ├── url: String
    │       ├── client: reqwest::Client          (reused across refreshes)
    │       ├── keys: RwLock<HashMap<kid → (DecodingKey, Algorithm)>>
    │       ├── refresh_ctrl: Mutex<RefreshCtrl>
    │       │   ├── consecutive_failures: u32
    │       │   └── backoff_until: Option<Instant>
    │       ├── max_failed_refreshes: u32
    │       ├── backoff_factor_secs: u64
    │       ├── max_age_secs: u64               (default 86400; 0 = disabled)
    │       └── expiry_task: OnceLock<AbortHandle>
    ├── expected_issuer: String
    ├── expected_audience: String
    ├── scope_claim_name: String
    └── mask_subject_in_logs: bool
```

## Request Flow (Cache-Miss Path)

```text
intercept(req)
  → validate_jwt(token)         [async]
    → decoding_key_for(token)   [async]
      → extract_kid(token)
      → keys.read().get(kid)    ← fast path: read lock, clone, release
           hit  → return (DecodingKey, Algorithm)
           miss → try_refresh(kid=Some(kid)).await
                    → refresh_ctrl.lock().await  (serialises all refreshes)
                    → keys.read().contains(kid)?  ← re-check after lock
                         hit  → return Ok(())     (expiry task refreshed first)
                         miss → enforce DoS controls
                                  failures >= max  → log error, Err
                                  now < backoff    → log warn, Err
                                → fetch_and_parse_jwks().await
                                    ok  → keys.write() = new_map; reset ctrl
                                    err → incr failures; set backoff_until; Err
              → keys.read().get(kid)  ← post-refresh check
                   hit  → return (DecodingKey, Algorithm)
                   miss → Err unauthenticated "Unknown JWT kid"
```

## Expiry Task Flow

```text
spawn run_expiry_loop(Weak<AuthzInterceptorInner>)   [on startup if max_age > 0]
  loop:
    sleep(compute_expiry_deadline(max_age_secs))     ← with ±1 % jitter
    Weak::upgrade() → None?  exit task               ← server shut down
    try_refresh(kid=None).await
      → refresh_ctrl.lock().await
      → DoS controls (failures / backoff)
      → fetch_and_parse_jwks().await
          ok  → keys.write() = new_map; reset ctrl; log info
          err → incr failures; set backoff_until; log error
```

## Expiry Deadline Calculation

```text
jitter_range = max_age_secs / 100            // 1 % of max_age
sign         = rand choice of {-1, +1}
jitter       = rand(0 ..= jitter_range) * sign
sleep        = max_age_secs + jitter         // lies in [0.99·max_age, 1.01·max_age]
```

For the default 86400 s (1 day), jitter is ±864 s (~14 min), sufficient to
spread refreshes across a fleet without meaningfully advancing or delaying the
effective rotation window.

## Design Decisions

### 1. Inline async refresh for cache miss, background task for expiry

`tonic_middleware::RequestInterceptor::intercept` is already `async`, so
`validate_jwt` and `decoding_key_for` can be made `async` too. This lets us
use `tokio::sync` primitives without `block_in_place` hacks.

Cache-miss refresh is inline: the caller waits for the result before the
request continues. Expiry refresh is proactive: it happens in a background
tokio task so it runs even when there is no incoming traffic. Combining both
in the same background task would delay the cache-miss response unnecessarily.

### 2. Shared `Mutex<RefreshCtrl>` serialises both triggers

One mutex governs both the cache-miss and expiry paths. Consequences:

- Only one HTTP fetch is ever in flight at a time, regardless of how many
  cache-miss requests are queued or whether the expiry task fires concurrently.
- Failure count and backoff state are shared. A broken JWKS endpoint
  suppresses retries from both triggers under the same backoff — no
  double-hammering.
- If both triggers fire simultaneously (cache expired AND new kid), the first
  through the mutex fetches; the second re-checks the map and exits early.

### 3. Double-checked locking on cache miss

The fast path holds only a `RwLock` read guard for the map lookup — no
contention with other reads. On a miss, the slow path acquires
`Mutex<RefreshCtrl>` and immediately re-checks the map under a new read lock.
This handles the common race where multiple concurrent requests miss the same
new kid: only the first one fetches; the rest find the key populated when they
acquire the mutex.

### 4. Owned `DecodingKey` returned from lookup

The original `decoding_key_for` returned `&'a DecodingKey` tied to the
`Arc<AuthzInterceptorInner>` lifetime. With the map behind an async `RwLock`,
the guard cannot outlive an await point. Cloning the `(DecodingKey, Algorithm)`
pair on lookup is the clean fix: `DecodingKey` implements `Clone`
(jsonwebtoken v10), the clone is cheap relative to network I/O, and it
removes the lifetime coupling entirely.

### 5. Background task lifecycle via `Weak` + `OnceLock<AbortHandle>`

The expiry task must not extend the server's lifetime, and must be cancelled
cleanly on shutdown. The approach:

- The task closure holds `Weak<AuthzInterceptorInner>`. When the server drops
  all `AuthzInterceptor` clones, the `Arc` strong count reaches zero and
  `Weak::upgrade()` returns `None`, causing the task to exit its loop.
- `JwksCache` stores the `AbortHandle` in a `std::sync::OnceLock`, set once
  immediately after the `Arc<AuthzInterceptorInner>` is constructed. The
  handle is needed because the Arc cannot be created and mutated in the same
  step.
- `JwksCache` implements `Drop` to call `handle.abort()`, ensuring the task
  is cancelled even if the server exits without waiting for the loop iteration
  to complete.

### 6. Expiry jitter ±1% of max_age

The info.md calls for "a small random amount". A fixed ±1% of max_age is
chosen because:
- It scales with the configured interval (±14 min for 1 day, ±6 s for 10 min).
- It is small enough not to meaningfully delay rotation pickup or advance
  unnecessary churn.
- It is large enough to spread refreshes across a fleet of servers.

Jitter is applied symmetrically (positive or negative) so the long-run average
refresh interval equals max_age.

### 7. Max-attempts gate takes priority over backoff

When `consecutive_failures >= max_failed_refreshes`, no further refresh is
attempted regardless of trigger. The server retains its last good cache and
logs an error on each subsequent unknown-kid request. Recovery requires a
restart. This is intentional: a hard gate is better than indefinite silent
retries against a broken endpoint.

### 8. Successful refresh resets all state

A successful fetch resets `consecutive_failures` to 0 and clears
`backoff_until`. Transient JWKS endpoint outages self-heal once the endpoint
recovers, without a restart.

### 9. `reqwest::Client` stored in `JwksCache`

The startup fetch currently builds a one-shot client that is dropped after
use. Storing the client in `JwksCache` avoids re-construction cost on each
refresh and reuses connection pools. The same 10-second timeout applies.

### 10. Config fields and defaults

| Field                        | Type  | Default | Purpose                                       |
| ---------------------------- | ----- | ------- | --------------------------------------------- |
| `jwks_max_failed_refreshes`  | `u32` | 5       | Successive failures before gate fires         |
| `jwks_backoff_factor_secs`   | `u64` | 3       | Exponential backoff base multiplier           |
| `jwks_max_age_secs`          | `u64` | 86400   | Cache lifetime in seconds; 0 disables expiry  |

All three fields are silently ignored when `jwt_public_key` is used instead of
`jwks_url`.

### 11. Backoff algorithm — equal jitter exponential

```text
temp  = min(cap, base * factor^attempt)   // cap = 300 s, base = 1 s
delay = temp/2 + rand(0 .. temp/2)        // equal jitter
```

Where `attempt` is zero-indexed (`consecutive_failures - 1` after the
increment). Requires the `rand` crate (not yet a project dependency).

## Testing Strategy

### Cache-miss integration test

Cache-miss refresh is tested against the live Hydra instance (the same one
used by the existing `server_integration_tests.rs` suite). The test uses
`ory_hydra_client::apis::jwk_api` — already a project dependency — to
manipulate dedicated Hydra key sets directly via the admin API. No additional
mock HTTP dependency is needed.

#### Test isolation

Hydra exposes custom key sets at `/admin/keys/{set}` independently of
`/.well-known/jwks.json` (which reflects only Hydra's internal signing keys
and is used by all other Hydra-backed tests). Each test creates a **dedicated
key set** with a unique name (e.g. `udex-jwks-refresh-{test-id}`) and
configures the test server to use `{hydra_admin_url}/admin/keys/{set}` as its
`jwks_url`.

This means:
- Other concurrent tests using `/.well-known/jwks.json` are entirely unaffected
  regardless of what the refresh tests do to their own key set.
- The test has full create/read/update/delete control over its own key set.
- Each test cleans up by calling `delete_json_web_key_set` on its own set.

Because the test server uses a custom set (not Hydra's signing keys), tokens
cannot be obtained from Hydra's `/oauth2/token` endpoint — those are signed
with Hydra's internal keys, not the custom set. Instead, the test **mints its
own JWTs** using a private key it generates and registers in the custom set.
This is the same pattern as the existing `authz` unit tests.

#### Test scenario (cache-miss)

1. Test creates custom key set containing key K1; starts server with `jwks_url`
   pointing at the custom set (caches K1).
2. Test mints a JWT signed with K1's private key; server validates successfully.
3. Test calls `jwk_api` to add key K2 to the custom set.
4. Test mints a JWT signed with K2's private key; server detects unknown `kid`,
   refreshes from the custom set, sees both K1 and K2, and validates.
5. Subsequent K2-signed tokens succeed without a second refresh; K1-signed
   tokens continue to work.

#### Test scenario (endpoint failure / cache retained)

1. Test deletes all keys from the custom set (causing `parse_jwks` to return
   an error on refresh — no usable keys with a `kid`).
2. A JWT signed with the previously cached key is presented; server attempts
   refresh, fails, retains the old cache, and validates the token using the
   still-cached key.
3. Error is logged; subsequent requests for cached keys continue to succeed.

#### Test scenario (max failed attempts gate)

1. Server configured with `jwks_max_failed_refreshes = 2`.
2. All keys deleted from the custom set so every refresh attempt fails.
3. Cache-miss requests are made until the gate fires.
4. Confirm no further HTTP fetches are attempted; server continues serving
   cached keys.

Tests live in a new file `projects/rust/server/tests/jwks_refresh_tests.rs`
to keep the refresh scenarios self-contained and separate from the existing
Hydra OAuth2 fixture.

### Alternatives considered for integration test approach

**`wiremock` mock HTTP server** — rejected. A controllable in-process mock
server requires adding a new dev dependency and hand-crafting JWKS JSON
responses. Using the real Hydra admin API instead tests the actual production
JWKS format, exercises the real HTTP fetch path, and reuses existing
infrastructure with zero new dependencies.

**Manipulating Hydra's internal signing key set** — rejected. Hydra's internal
signing keys are shared across all tests via `/.well-known/jwks.json`. Adding
or removing keys from that set during a test run would interfere with any
concurrent test that validates tokens, making the suite non-parallelisable.
Dedicated custom key sets provide the same test coverage with full isolation.

## Alternatives Considered

### Lazy expiry check on each request

Check whether the cache is older than max_age on every request, and refresh
inline if so. Rejected because:
- The cache goes stale during idle periods and is not refreshed until the next
  request, which may be arbitrarily late.
- It adds a wall-clock comparison to every authenticated request.
A background task refreshes on schedule regardless of traffic.

### Non-blocking cache-miss refresh (fail-fast, retry on next request)

Spawn a background task on cache miss and return `UNAUTHENTICATED` immediately.
Rejected because:
- Every token with a new kid triggers a transient failure; clients must
  implement retry logic.
- Inline refresh holds the request for one network round-trip (≤ 10 s) and
  returns a definitive result — better UX and simpler client requirements.

### Independent DoS state per trigger

Track failure count and backoff separately for cache-miss and expiry paths.
Rejected because a broken JWKS endpoint would receive double the retries with
no benefit, and the two paths would reach different gate states independently,
making observability harder.

### `std::sync` primitives + `block_in_place`

Use `std::sync::RwLock` and `std::sync::Mutex` and wrap the fetch in
`tokio::task::block_in_place`. Rejected because `intercept` is already
`async`; `tokio::sync` is cleaner and avoids blocking a worker thread during
the fetch.
