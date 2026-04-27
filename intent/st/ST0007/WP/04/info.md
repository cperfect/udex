---
verblock: "27 Apr 2026:v0.1: vscode - Initial version"
wp_id: WP-04
title: "Extend server_integration_tests.rs with Hydra-JWKS fixture and scope/token-reuse tests"
scope: Small
status: Not Started
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

- [ ] All pre-existing integration tests pass in static-PEM mode.
- [ ] Hydra-JWKS fixture starts a server and issues/validates real Hydra JWTs
      when `HYDRA_ADMIN_URL`/`HYDRA_PUBLIC_URL` are set.
- [ ] Scope sub-set test passes (Hydra only).
- [ ] Token re-use test passes (both fixtures).
- [ ] Expired token test passes (both fixtures).
- [ ] Tests that need Hydra are skipped gracefully when Hydra is not available.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and
      the full `cargo test -p udex-server` suite pass.

## Dependencies

Depends on WP-01 (JWKS interceptor), WP-02 (config validation), and WP-03
(auth_server helper). Must be the last WP implemented.
