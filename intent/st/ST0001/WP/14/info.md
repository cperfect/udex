---
verblock: "06 Apr 2026:v0.1: vscode - Initial version"
wp_id: WP-14
title: "Remove duplicate JWT warning log test"
scope: Small
status: Not Started
priority: minor
---

# WP-14: Remove duplicate JWT warning log test

## Review Finding

🟡 **Minor** — `test_invalid_jwt_emits_warn` and `test_jwt_wrong_issuer_emits_warn` in `server/src/authn.rs` both trigger the same `tracing::warn!` in `validate_jwt()` and assert the same log message. The second test name implies it tests a distinct scenario (wrong issuer), but the token `"eyJhbGciOiJFUzI1NiJ9.eyJzdWIiOiJ4In0.invalidsig"` fails at signature validation, not issuer validation — so both tests exercise the exact same code path.

## Objective

Remove the duplication. Either:
1. Delete `test_jwt_wrong_issuer_emits_warn` as a genuine duplicate, or
2. Replace it with a test that uses a properly-signed JWT with a wrong issuer to genuinely exercise the issuer validation path.

## Option 2 approach

Generate a JWT signed with `tests/jwt/signing_private_key.pem` but with `iss` set to `"wrong-issuer"`. This would pass signature validation but fail issuer validation, confirming the warn fires for that path too.

## Acceptance Criteria

- [ ] No two tests in `authn::tests` assert the exact same log message via the same code path without testing a genuinely distinct scenario

## Dependencies

- None
