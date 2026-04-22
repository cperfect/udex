---
verblock: "22 Apr 2026:v0.1: vscode - Initial version"
wp_id: WP-01
title: "Add scope field to Claims struct, remove extra map"
scope: Small
status: Not Started
---

# WP-01: Add scope field to Claims struct, remove extra map

## Objective

Add a `scope` field to `Claims` in `udex_api::authz::claims` so the RFC 8693
`scope` claim is deserialised as a first-class field, and remove the `extra`
HashMap (along with `add_extras()` and `get_extras()`) which existed solely to
carry the old `permissions` claim.

## Deliverables

- `pub scope: String` field on `Claims` with `#[serde(default)]`
- `with_scope(scope: impl Into<String>) -> Self` builder on `Claims`
- Removal of the `#[serde(flatten)] extra` field, `add_extras()`, and `get_extras()`
- Updated / replaced unit tests in `claims.rs`

## Notes

- Serde ignores unknown JWT fields by default on a plain struct (no
  `deny_unknown_fields`), so removing `#[serde(flatten)]` does not break
  deserialization of tokens carrying `openid`, `email`, `name`, or any other
  non-`scope` claims — they are simply discarded.
- The tests in `claims.rs` that use `add_extras` to set `permissions` will be
  removed here and replaced with `scope`-based equivalents.

## Acceptance Criteria

- [ ] `Claims` has `pub scope: String` with `#[serde(default)]`.
- [ ] Deserialising a JWT payload without a `scope` key yields `scope = ""` (no error).
- [ ] Serialising a `Claims` with a non-empty `scope` produces `"scope": "..."` in the payload.
- [ ] Deserialising a JWT payload with unknown fields (e.g. `email`, `name`) succeeds and those fields are ignored.
- [ ] `Claims::new()` signature is unchanged.
- [ ] `with_scope()` builder method added for constructing scoped claims in tests.
- [ ] `extra` field, `add_extras()`, and `get_extras()` are fully removed.
- [ ] All `claims.rs` unit tests updated — no references to `permissions` extra claim remain.
- [ ] `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, and `cargo test -p udex-api` pass.

## Dependencies

None — this is the foundation for WP-02 and WP-03.
