---
verblock: "16 May 2026:v0.1: vscode - Initial version"
wp_id: WP-01
title: "Fix LookupKeyByContextOrCreate — require read + write"
scope: Small
status: Done
---

# WP-01: Fix LookupKeyByContextOrCreate — require read + write

## Objective

`LookupKeyByContextOrCreate` both reads (checks whether an entry exists) and writes
(creates one if not found). Its `Permissable` impl currently requires only
`udex:entry:v1:{index}:write`. Add `udex:entry:v1:{index}:read` as a second required
permission so that callers must hold both grants explicitly.

## Deliverables

- `api/src/authz/entry.rs` — update `Permissable<LookupKeyByContextOrCreateRequest>` to
  return `[read, write]`; update unit tests (`test_lookup_or_create_with_write_permission`
  → now requires both, `test_lookup_or_create_read_permission_denied` is still denied but
  for a different reason — write-only is also insufficient)
- `server/tests/entry_service_integration_tests.rs` — update any test that mints a
  write-only token for `lookup_or_create` paths
- `sdk/tests/integration_tests.rs` — update `lookup_or_create` test tokens
- `cli/tests/entry_live_tests.rs` — update the test JWT scope from `write` to `read write`

## Acceptance Criteria

- [ ] `Permissable` impl returns `[udex:entry:v1:{index}:read, udex:entry:v1:{index}:write]`
- [ ] Caller with only `write` is denied with `PermissionDenied`
- [ ] Caller with only `read` is denied with `PermissionDenied`
- [ ] Caller with both `read` and `write` succeeds
- [ ] All existing tests pass with the updated scopes
- [ ] `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test` all pass

## Dependencies

- None; WP-02 should be done after this so `BulkWrite` can delegate to the fixed
  `LookupKeyByContextOrCreate.required_permissions()`
