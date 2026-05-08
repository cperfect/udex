---
verblock: "08 May 2026:v0.1: vscode - Initial version"
wp_id: WP-03
title: "OAuth2 client-credentials token lifecycle"
scope: Small
status: Not Started
---

# WP-03: OAuth2 client-credentials token lifecycle

## Objective

Implement transparent OAuth2 client-credentials token acquisition and refresh so callers never handle tokens manually — the SDK injects a valid Bearer token into every RPC.

## Deliverables

- `TokenManager`: fetches tokens from the Hydra token endpoint, caches them, and proactively refreshes before expiry
- `ClientOptions` extended with `client_id`, `client_secret`, `token_url`, `audience`, `scope`
- tonic `Interceptor` (or `tower` middleware) that injects `Authorization: Bearer <token>` per request
- In-memory only — no keychain or persistent storage

## Acceptance Criteria

- [ ] An RPC made with valid credentials succeeds against the compose stack
- [ ] Token is refreshed automatically when it expires (unit test with mock clock / short-lived token)
- [ ] Missing or expired credentials return a typed `Error::Auth` variant, not a raw tonic status

## Dependencies

- WP-02
