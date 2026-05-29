---
verblock: "29 May 2026:v0.1: vscode - Initial version"
wp_id: WP-04
title: "Integration tests"
scope: Medium
status: Done
---

# WP-04: Integration tests

## Objective

End-to-end integration test coverage for both refresh triggers against a
live Hydra instance, with full isolation from other concurrent Hydra tests.

## Test Isolation Strategy

Hydra exposes custom key sets at `/admin/keys/{set}` independently of
`/.well-known/jwks.json` (which reflects only Hydra's internal signing keys).
Each test creates a **dedicated key set** (e.g. `udex-jwks-refresh-{unique-id}`)
and configures the test server to use `{hydra_admin_url}/admin/keys/{set}` as
its `jwks_url` (with `danger_allow_non_tls: true`).

Consequences:
- Other concurrent tests using `/.well-known/jwks.json` are entirely unaffected.
- The test has full read/write control over its own key set.
- The test mints its own JWTs signed with private keys it generates, matching
  the public keys in the custom set (same pattern as existing unit tests).
- Cleanup simply calls `delete_json_web_key_set` on the custom set.

No new dependencies are needed — `ory_hydra_client::apis::jwk_api` and
`ory_hydra_client::apis::jwk_api::create_json_web_key_set` are already in
the project.

## Deliverables

- New file `projects/rust/server/tests/jwks_refresh_tests.rs`
- Helper `create_test_jwks_set(admin_url, set_name) -> (JwkSet, EncodingKey)`
  that creates a named key set in Hydra and returns the set and the matching
  private signing key for JWT minting
- Test fixture `init_server_jwks_refresh` — starts a server whose `jwks_url`
  points to a dedicated Hydra custom key set
- Test: **key rotation → cache-miss → refresh → success**
  Add K1 to the custom set; start server (caches K1); add K2; mint a JWT
  signed with K2's private key; server detects unknown `kid`, refreshes,
  sees K2, validates successfully; K1-signed tokens still work
- Test: **JWKS endpoint unavailable → cache retained, errors logged**
  Delete all keys from the custom set (causing `parse_jwks` to return an
  error on refresh); confirm the server retains cached keys, serves requests
  for the previously known kid, and logs refresh errors
- Test: **max failed attempts gate**
  Delete all keys and set `jwks_max_failed_refreshes = 2` on the test server;
  trigger enough cache-miss attempts to exhaust the gate; confirm no further
  refreshes are attempted and the server continues serving cached keys

## Acceptance Criteria

- [ ] All three test scenarios pass against a running Hydra instance
      (gated by `HYDRA_ADMIN_URL` env var, consistent with existing suite)
- [ ] Tests skip gracefully when Hydra is unavailable
- [ ] Each test cleans up its dedicated key set on completion (including on
      failure via RAII or explicit teardown)
- [ ] Running the refresh tests concurrently with the existing Hydra tests
      produces no failures in either suite
- [ ] `cargo clippy --all-targets -- -D warnings` clean

## Dependencies

- WP02 (cache-miss refresh path must be implemented)
- WP03 (expiry task must be implemented, even if not directly tested here)
