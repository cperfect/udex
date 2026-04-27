---
verblock: "27 Apr 2026:v0.1: vscode - Initial version"
wp_id: WP-04
title: "Extend server_integration_tests.rs with Hydra-JWKS fixture and scope/token-reuse tests"
scope: Small
status: Done
---

# WP-04: Extend server_integration_tests.rs with Hydra-JWKS fixture and scope/token-reuse tests

## Objective

Extend `server_integration_tests.rs` with a second server fixture backed by
Hydra JWKS, and add tests covering scope sub-set authentication, token re-use,
and expired token rejection. All existing static-PEM tests must continue to
pass unchanged.

## Deliverables

- `init_server_hydra()` async fixture function: starts a server configured with
  `jwks_url` pointing at the local Hydra instance; creates a shared OAuth2
  client via `auth_server::create_oauth2_client`; returns an equivalent
  `MaybeOnceType` tuple extended with the Hydra client config.
- Fixture is activated only when `HYDRA_ADMIN_URL` and `HYDRA_PUBLIC_URL` env
  vars are set; tests that depend on it call `tokio::test::skip_if` (or
  equivalent) when the vars are absent.
- New test cases (run against both fixtures where applicable, Hydra-only where
  not):
  - **Scope sub-set**: authenticate with a strict subset of client scopes;
    assert the server accepts the token and only those scopes are present in
    the request extensions.
  - **Token re-use**: obtain one token, make two sequential requests with it;
    both succeed.
  - **Expired token**: craft a token with `exp` in the past; assert
    `UNAUTHENTICATED` is returned.
  - **Wrong audience** (Hydra variant): authenticate against Hydra with a
    different audience; assert `UNAUTHENTICATED`.
- `AuthNzConfig` construction in the static-PEM fixture updated to use the new
  validated form (no `jwt_public_key_path` + `jwks_url` conflict).

## Acceptance Criteria

- [x] All pre-existing integration tests pass in static-PEM mode.
- [x] Hydra-JWKS fixture starts a server and issues/validates real Hydra JWTs
      when `HYDRA_ADMIN_URL`/`HYDRA_PUBLIC_URL` are set.
- [x] Scope sub-set test passes (Hydra only) — `test_hydra_scope_subset`.
- [x] Token re-use test passes (both fixtures) — `test_token_reuse` (static PEM)
      and `test_hydra_token_reuse` (Hydra).
- [x] Expired token test passes (static PEM) — covered by tests 11, 13, 15 in
      `test_authz`. Hydra variant not implemented: Hydra-issued tokens carry a
      real expiry and cannot be backdated without Hydra config changes; replaced
      by `test_hydra_wrong_audience_rejected` (UNAUTHENTICATED) and
      `test_hydra_non_udex_scopes_denied` (PermissionDenied) as equivalent
      negative-path coverage.
- [x] Tests that need Hydra are skipped gracefully when Hydra is not available.
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and
      the full `cargo test -p udex-server` suite pass.

## Dependencies

Depends on WP-01 (JWKS interceptor), WP-02 (config validation), and WP-03
(auth_server helper). Must be the last WP implemented.
