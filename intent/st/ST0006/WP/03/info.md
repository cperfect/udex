---
verblock: "22 Apr 2026:v0.1: vscode - Initial version"
wp_id: WP-03
title: "Update integration tests to use scope claim"
scope: Small
status: Done
---

# WP-03: Update integration tests to use scope claim

## Objective

Update all JWT generation in the server integration tests to use the `scope`
claim instead of the `permissions` extra claim, and add a test verifying that
non-`udex:` scopes in a mixed token are silently ignored.

## Deliverables

- `OverrideClaims` struct in `server_integration_tests.rs` updated: replace
  `extra` permissions usage with a `scope: Option<String>` field
- `generate_test_claims()` updated to set `claims.scope` from the override
- All existing test token construction sites migrated from
  `extra["permissions"]` to `scope`
- New test: a token with `scope = "openid profile email udex:index:v1:{name}:read"`
  successfully authorises only the `read` operation

## Acceptance Criteria

- [ ] `OverrideClaims` has a `scope: Option<String>` field replacing permission-carrying extras.
- [ ] All existing authNZ integration tests pass unchanged in behaviour.
- [ ] New mixed-scope test passes: non-`udex:` scopes are ignored, `udex:` scope grants access.
- [ ] No references to the old `permissions` extra-claim remain in test code.
- [ ] `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, and the full integration test suite pass.

## Dependencies

- WP-01 (scope field on Claims)
- WP-02 (permission extraction reads scope)
