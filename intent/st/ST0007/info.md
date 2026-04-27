---
verblock: "27 Apr 2026:v0.1: vscode - Initial version"
intent_version: 2.4.0
status: WIP
slug: integrated-oauth2-authorization-server
created: 20260427
completed:
---

# ST0007: Integrated OAuth2 Authorization Server

## Objective

Replace the static-PEM-key authentication path with a production-ready approach
that supports both a local PEM key file and a remote JWKS endpoint. Wire up an
Ory Hydra instance in Docker Compose as the reference OAuth2 server for
development and integration testing. Only the `client_credentials` grant is in
scope.

## Context

The server currently validates JWTs using a single EC public key read from a
PEM file at startup (`jwt_public_key_path`). This is incompatible with real
OAuth2 servers, which expose a JWKS endpoint and rotate keys identified by
`kid`. To work with Hydra (or any standards-compliant OAuth2 server) the
`AuthnInterceptor` needs to:

1. Fetch and cache the JWKS at startup.
2. Select the correct `DecodingKey` based on the `kid` header in each token.
3. Continue to support the static PEM path for environments that don't run
   Hydra (e.g., the existing CI path that issues self-signed tokens).

Hydra is already added to Docker Compose (`83d2f54`) and the dev container is
configured. There are two WIP commits with scaffolding:
- `authn.rs`: incomplete `jwks_url` match arm and syntax errors
- `tests/auth_server.rs`: incomplete helper for creating Hydra clients and
  obtaining tokens via `client_credentials`

The non-test Rust code must have **no dependency on Ory Hydra** — it speaks
standard OAuth2/JWKS only.

## Scope

- Fix and complete `AuthnInterceptor` to support both `jwt_public_key_path` and
  `jwks_url` config paths, with `kid`-based key selection for JWKS.
- Update `AuthNzConfig::validate()` to accept `jwks_url` as an alternative to
  `jwt_public_key_path`.
- Complete `tests/auth_server.rs` as a reusable test-only helper (Hydra client
  creation + `client_credentials` token exchange) — not compiled into the server
  binary.
- Update `server_integration_tests.rs` to run under two modes:
  - **Static PEM mode** (existing): issues self-signed tokens using local keys.
  - **Hydra JWKS mode** (new): creates OAuth2 clients in Hydra, authenticates,
    and uses the resulting JWTs. Activated when a Hydra endpoint is reachable
    (environment variable or feature flag).
- Existing tests must continue to pass in both modes.
- Scope-based permission tests must not require a new Hydra client per scenario;
  authenticate with a subset of the client's scopes instead.
- Consider token re-use / caching tests (e.g., same token accepted twice,
  expired token rejected).

## Out of Scope

- Authorization Code, PKCE, Device, or any other OAuth2 grant type.
- Dynamic key rotation (JWKS refresh after initial fetch is a future concern).
- Any Hydra-specific API calls in non-test production code.

## Related Steel Threads

- ST0003 — original AuthNZ implementation
- ST0006 — RFC 8693 scope claim (the `scope` field that Hydra will populate)

## Acceptance Criteria

- [ ] `AuthnInterceptor::new()` compiles and handles both `jwt_public_key_path`
      and `jwks_url`; exactly one must be set or `ConfigValidation` error is
      returned.
- [ ] JWKS path: fetches the JWKS URL at startup, builds a `kid → DecodingKey`
      map, and selects the correct key per token.
- [ ] Static PEM path: behaviour unchanged from pre-ST0007.
- [ ] `AuthNzConfig::validate()` accepts either field (not both, not neither).
- [ ] `tests/auth_server.rs` compiles; provides `create_oauth2_client` and
      `authenticate` helpers (Hydra-backed, test-only).
- [ ] Integration tests pass in static-PEM mode (existing CI).
- [ ] Integration tests pass in Hydra-JWKS mode when Hydra is available.
- [ ] Scope sub-set authentication tested: client created with scopes A+B,
      token requested with scope A only — server accepts and grants only A.
- [ ] Token re-use test: same unexpired token accepted on a second request.
- [ ] Expired token test: token with past `exp` is rejected with UNAUTHENTICATED.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, all
      tests pass.
