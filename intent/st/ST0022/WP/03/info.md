---
verblock: "29 May 2026:v0.1: vscode - Initial version"
wp_id: WP-03
title: "Expiry background task"
scope: Small
status: Not Started
---

# WP-03: Expiry background task

## Objective

Implement the proactive expiry-based refresh: a tokio background task that
sleeps until the next expiry deadline (with ±1% jitter) and then calls
`try_refresh` via the shared `RefreshCtrl`. The task must not extend the
server lifetime and must cancel cleanly on shutdown.

## Deliverables

- `compute_expiry_deadline(max_age_secs) -> Duration` — returns
  `max_age_secs ± rand(0..=1% of max_age_secs)`; uses `rand`
- `run_expiry_loop(weak: Weak<AuthzInterceptorInner>)` — free async function:
  loops sleeping then calling `try_refresh(kid=None)`; exits when
  `Weak::upgrade()` returns `None`
- Task spawned in `AuthzInterceptor::new` immediately after the
  `Arc<AuthzInterceptorInner>` is constructed, iff `max_age_secs > 0`;
  `AbortHandle` stored in `JwksCache::expiry_task` via `OnceLock::set`
- `Drop` impl on `JwksCache` (scaffolded in WP01) confirmed to abort the
  handle on drop
- Remove `#[allow(dead_code)]` from the remaining scaffold fields consumed here

## Acceptance Criteria

- [ ] Unit test: `compute_expiry_deadline` output lies within
      `[0.99 * max_age_secs, 1.01 * max_age_secs]` across many samples
- [ ] Unit test: the expiry loop exits promptly when the `Weak` reference
      can no longer be upgraded (simulates server shutdown)
- [ ] Unit test: `max_age_secs = 0` does not spawn a task
- [ ] `cargo clippy --all-targets -- -D warnings` clean
- [ ] All existing unit tests continue to pass

## Dependencies

- WP01 (`JwksCache` struct, `OnceLock<AbortHandle>`, `Drop` impl)
- WP02 (`try_refresh` method — the loop calls it)
