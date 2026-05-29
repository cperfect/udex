---
verblock: "29 May 2026:v0.1: vscode - Initial version"
wp_id: WP-04
title: "Integration tests"
scope: Medium
status: Not Started
---

# WP-04: Integration tests

## Objective

End-to-end integration test coverage for both refresh triggers against a
live Hydra instance. Tests use `ory_hydra_client::apis::jwk_api` (already
a project dependency) to manipulate Hydra's JWK key sets directly via the
admin API — no new dependencies required.

## Deliverables

- New file `projects/rust/server/tests/jwks_refresh_tests.rs`
- Test fixture `init_server_jwks_refresh` — starts a server backed by a
  Hydra JWKS endpoint, similar to the existing `init_server_hydra` fixture
- Test: **key rotation → cache-miss → refresh → success**
  Use `jwk_api` to add a new key to Hydra's signing key set; obtain a
  token signed with the new key; confirm the server detects the unknown `kid`,
  refreshes, and validates the token successfully
- Test: **JWKS endpoint unavailable → cache retained**
  Simulate an unreachable JWKS endpoint (or Hydra returning an error);
  confirm the server retains cached keys, serves requests for known kids,
  and logs errors
- Test: **max failed attempts gate**
  Force enough consecutive refresh failures to trigger the gate; confirm the
  server stops attempting refreshes and continues serving existing cached keys

## Acceptance Criteria

- [ ] All three test scenarios pass against a running Hydra instance
      (gated by the `HYDRA_ADMIN_URL` env var, consistent with existing suite)
- [ ] Tests skip gracefully when Hydra is unavailable
- [ ] `cargo clippy --all-targets -- -D warnings` clean
- [ ] No regressions in existing integration tests

## Dependencies

- WP02 (cache-miss refresh path must be implemented)
- WP03 (expiry task must be implemented, even if not directly tested here)
