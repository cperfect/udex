# Tasks - ST0007: Integrated OAuth2 Authorization Server

## Work Packages

- [ ] WP-01: Fix AuthnInterceptor — JWKS startup fetch and kid-based key selection
- [ ] WP-02: Update AuthNzConfig validation — accept jwks_url as alternative to jwt_public_key_path
- [ ] WP-03: Complete auth_server.rs test helper — Hydra client creation and client_credentials token exchange
- [ ] WP-04: Extend server_integration_tests.rs — Hydra-JWKS fixture and scope/token-reuse tests

## Dependencies

WP-01 and WP-02 can be done in parallel.
WP-03 can be developed in parallel at code level but is exercised end-to-end only after WP-01.
WP-04 depends on WP-01, WP-02, and WP-03 all being complete.
