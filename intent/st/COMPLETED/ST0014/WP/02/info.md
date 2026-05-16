---
verblock: "16 May 2026:v0.1: vscode - Initial version"
wp_id: WP-02
title: "Fix BulkWriteEntryOperation — derive permissions from contained operations"
scope: Small
status: Done
---

# WP-02: Fix BulkWriteEntryOperation — derive permissions from contained operations

## Objective

`BulkWriteEntryOperation` accepts a heterogeneous list of operations (`CreateEntry`,
`DeleteEntry`, `LookupOrCreate`). Its `Permissable` impl currently requires a blanket
`udex:entry:v1:{index}:write` regardless of what is actually in the request. Replace this
with dynamic derivation: iterate the operations, call each operation type's own
`required_permissions()`, and return the deduplicated union.

## Deliverables

- `api/src/authz/entry.rs` — update `Permissable<BulkWriteEntryOperationRequest>` to
  iterate `self.operations`, match on the `oneof operation` variants (`CreateEntry`,
  `DeleteEntry`, `LookupOrCreate`), collect the union of each variant's
  `required_permissions()`, and return it deduplicated
- `api/src/authz/entry.rs` — update or add unit tests covering: create-only bulk (needs
  `create`), delete-only bulk (needs `delete`), lookup-or-create bulk (needs `read` +
  `write` after WP-01), mixed bulk (needs union of all)
- `server/tests/entry_service_integration_tests.rs` — verify the bulk write integration
  test uses a token with the correct scopes for its operations

## Acceptance Criteria

- [x] A bulk request containing only `CreateEntry` ops requires only `create`
- [x] A bulk request containing only `DeleteEntry` ops requires only `delete`
- [x] A bulk request containing `LookupOrCreate` ops requires `read` + `write`
- [x] A mixed request requires the union of all contained operations' permissions
- [x] An empty bulk request is handled by WP-03 (not in scope here)
- [x] `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test` all pass

## Dependencies

- WP-01 must be complete first so that `LookupKeyByContextOrCreate.required_permissions()`
  already returns `[read, write]` and this WP can compose correctly
