---
verblock: "29 May 2026:v0.1: vscode - Initial version"
wp_id: WP-02
title: "Cache-miss refresh"
scope: Medium
status: Done
---

# WP-02: Cache-miss refresh

## Objective

Implement the inline cache-miss refresh path: when `decoding_key_for`
encounters a `kid` not in the cache, trigger a JWKS fetch, apply DoS
controls, update the map on success, and return the key (or a definitive
error). No background task in this WP — that is WP03.

## Deliverables

- `JwksCache::fetch_and_parse_jwks` — async method that fetches the JWKS
  URL via the stored `client` and calls `parse_jwks`; returns the new key map
  or an error
- `JwksCache::try_refresh` — async method implementing the full slow path:
  - acquires `Mutex<RefreshCtrl>` (serialises all refreshes)
  - re-checks the map (double-checked locking — another task may have refreshed)
  - enforces DoS controls: `consecutive_failures >= max_failed_refreshes` gate,
    then `backoff_until` check
  - calls `fetch_and_parse_jwks`; on success writes the new map and resets
    `RefreshCtrl`; on failure increments failures and sets `backoff_until`
- `compute_backoff(consecutive_failures, factor_secs) -> Duration` —
  equal-jitter exponential backoff capped at 300 s; uses `rand`
- Cache-miss path wired into `decoding_key_for`: on `None` from the fast-path
  read, calls `try_refresh(kid).await` then re-checks the map
- Remove `#[allow(dead_code)]` from `RefreshCtrl` and the fields consumed here

## Acceptance Criteria

- [ ] Unit test: DoS gate fires after `max_failed_refreshes` consecutive failures
- [ ] Unit test: backoff suppresses retry attempts within the window
- [ ] Unit test: successful refresh resets `consecutive_failures` and `backoff_until`
- [ ] Unit test: concurrent requests all missing the same `kid` produce exactly
      one HTTP fetch (double-checked locking)
- [ ] Unit test: `compute_backoff` output lies within the equal-jitter bounds
      for several attempt values
- [ ] `cargo clippy --all-targets -- -D warnings` clean
- [ ] All existing unit tests continue to pass

## Dependencies

- WP01 must be complete (`JwksCache`, `parse_jwks`, async method signatures)
