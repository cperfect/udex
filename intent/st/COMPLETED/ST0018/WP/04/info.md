---
verblock: "18 May 2026:v0.1: vscode - Initial version"
wp_id: WP-04
title: "SDK test audit: port JWT-only cases to OAuth2 fixture"
scope: Small
status: Done
---

# WP-04: SDK test audit: port JWT-only cases to OAuth2 fixture

## Objective

Audit the SDK integration tests and port all general-API test cases that existed only under the static-JWT fixture to also run under the OAuth2 (Hydra) fixture, increasing real-world auth coverage.

## Deliverables

- `projects/rust/sdk/tests/integration_tests.rs` — 6 new OAuth2 test cases added; doc comment added to the one JWT-specific test that intentionally remains JWT-only.

## New OAuth2 tests added

- `test_sdk_oauth2_lookup_or_create_creates_new_entry`
- `test_sdk_oauth2_lookup_or_create_returns_existing_entry`
- `test_sdk_oauth2_envelope_encrypted_entry`
- `test_sdk_oauth2_delete_index_empty`
- `test_sdk_oauth2_delete_index_not_empty`
- `test_sdk_oauth2_delete_index_not_found`

## JWT-only test retained

- `test_sdk_invalid_token_returns_rpc_error` — exercises static bearer token rejection; the OAuth2 analogue (`test_sdk_oauth2_invalid_credentials_return_auth_error`) already existed and covers the token-acquisition failure path.

## Acceptance Criteria

- [x] `cargo test -p udex-sdk --test integration_tests --no-run` compiles cleanly
- [x] All new tests follow the `test_sdk_oauth2_*` naming convention
- [x] No existing tests removed or altered (coverage only increases)

## Dependencies

- None
