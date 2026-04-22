---
verblock: "22 Apr 2026:v0.1: vscode - Initial version"
wp_id: WP-02
title: "Rewrite permission extraction to use scope claim"
scope: Small
status: Done
---

# WP-02: Rewrite permission extraction to use scope claim

## Objective

Replace the custom `permissions` JSON-array extraction in
`udex_api::authz::permissions` with RFC 8693 `scope`-based extraction: split
`claims.scope` on whitespace, retain only `udex:`-prefixed values, and silently
discard everything else.

## Deliverables

- Updated `extract_permissions()` (or equivalent) in `api/src/authz/permissions.rs`
  reading from `claims.scope` instead of `claims.get_extras()["permissions"]`
- Removal of the old JSON-array parsing path
- Updated unit tests in `permissions.rs` covering:
  - scope with only `udex:` values
  - scope mixing `udex:` and non-`udex:` values (latter silently discarded)
  - empty / missing scope

## Acceptance Criteria

- [x] `extract_permissions()` reads `claims.scope`, splits on ASCII whitespace.
- [x] Only values prefixed with `udex:` are retained; all others are silently discarded (no log, no error).
- [x] The old `permissions` extra-claim path is fully removed.
- [x] `scope = "openid profile email udex:entry:v1:my-index:read"` yields exactly `["udex:entry:v1:my-index:read"]`.
- [x] `scope = ""` (or absent) yields an empty permission set — not an error.
- [x] Existing per-permission validation (regex, format) is preserved.
- [x] `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, and `cargo test -p udex-api` pass.

## Dependencies

- WP-01 (scope field on Claims must exist)
