---
verblock: "27 Apr 2026:v0.1: vscode - Initial version"
wp_id: WP-03
title: "Complete auth_server.rs test helper: Hydra client creation and client_credentials token exchange"
scope: Small
status: Done
---

# WP-03: Complete auth_server.rs test helper: Hydra client creation and client_credentials token exchange

## Objective

Complete `projects/rust/server/tests/auth_server.rs` into a compilable,
reusable test-only helper module. It must provide functions to create OAuth2
clients in Hydra (via the Hydra admin API) and exchange client credentials for
a JWT, with no Hydra-specific types leaking into callers.

## Deliverables

- `OAuthClientConfig` struct: `id`, `secret`, `scopes` (Vec<String>),
  `audience`.
- `create_oauth2_client(admin_url, client: OAuthClientConfig)` — calls Hydra
  admin API to register the client with `access_token_strategy = "jwt"`,
  `grant_types = ["client_credentials"]`. Returns `Result<(), Error>`.
- `authenticate(public_url, client: OAuthClientConfig, scopes: Vec<String>)`
  — performs `client_credentials` token exchange using the `oauth2` crate;
  `scopes` may be a subset of those on the client. Returns `Result<String, Error>`
  (the raw access token string).
- `API_CONFIG` must not use `todo!()` or hardcoded `const` initialisers for
  non-`Copy` types — construct it in a function.
- All `todo!()`, `None()` (invalid), and mutable-field-on-immutable-struct
  bugs from the WIP scaffold are fixed.
- Module is only compiled as part of integration tests (not the server binary).

## Acceptance Criteria

- [x] `tests/auth_server.rs` compiles with `cargo test -p udex-server
      --test auth_server` (even when Hydra is not running — compilation only).
- [x] `create_oauth2_client` creates a client against a live Hydra instance
      when `HYDRA_ADMIN_URL` is set.
- [x] `authenticate` returns a non-empty JWT string against a live Hydra
      instance when `HYDRA_PUBLIC_URL` is set.
- [x] Requesting a subset of client scopes succeeds; the returned token's
      `scope` claim contains only the requested scopes.
- [x] `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` pass.

## Dependencies

Depends on WP-01 (JWKS key selection) being in place before this helper is
exercised end-to-end in WP-04. Can be developed in parallel at code level.
