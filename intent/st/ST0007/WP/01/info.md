---
verblock: "27 Apr 2026:v0.1: vscode - Initial version"
wp_id: WP-01
title: "Fix AuthnInterceptor: JWKS startup fetch and kid-based key selection"
scope: Small
status: Done
---

# WP-01: Fix AuthnInterceptor: JWKS startup fetch and kid-based key selection

## Objective

Rewrite `AuthnInterceptor` in `udex_server::authn` to support two mutually
exclusive key sources: the existing static EC PEM file and a new JWKS URL.
When using JWKS, fetch the key set at startup, build a `kid → DecodingKey`
map, and select the correct key per token by reading the `kid` header claim.

## Deliverables

- `AuthnInterceptor` holds either a single `DecodingKey` (PEM path) or a
  `HashMap<String, DecodingKey>` (JWKS path) — internal enum or similar.
- At startup, JWKS path performs a blocking HTTP GET of `jwks_url`, parses the
  `JwkSet`, and builds the key map. Any failure is a `ConfigValidation` error.
- Per-request: decode JWT header (no signature check), read `kid`, look up key.
  Unknown or missing `kid` → `UNAUTHENTICATED`.
- Static PEM path: unchanged behaviour.
- Unit tests in `authn.rs` updated to cover both paths.
- Dependencies: add a blocking HTTP client (e.g. `reqwest` with
  `blocking` feature) to `[dependencies]` in `server/Cargo.toml`; keep
  `ory-hydra-client` in `[dev-dependencies]` only.

## Acceptance Criteria

- [ ] `AuthnInterceptor::new()` compiles with no `todo!()`, `unwrap()` on
      fallible paths, or syntax errors.
- [ ] Given `jwks_url`, fetches the endpoint at construction time and rejects
      an unreachable URL with `ConfigValidation`.
- [ ] Token signed by a key in the JWKS is accepted; token with unknown `kid`
      is rejected with `UNAUTHENTICATED`.
- [ ] Given `jwt_public_key_path`, behaviour is identical to pre-ST0007.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and
      `cargo test -p udex-server` (unit tests only) pass.

## Dependencies

None — this is the foundation for WP-03 and WP-04.
