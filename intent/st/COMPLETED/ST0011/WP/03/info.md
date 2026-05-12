---
verblock: "12 May 2026:v0.1: vscode - Initial version"
wp_id: WP-03
title: "Server handler and integration tests"
scope: Small
status: Done
---

# WP-03: Server handler and integration tests

## Objective

Implement the `delete_index` gRPC handler in the server and wire up authorization, following the exact same pattern as `create_index`.

## Deliverables

- `projects/rust/server/src/index.rs`:
  - Import `DeleteIndexRequest`, `DeleteIndexResponse` from `udex_api`
  - Add `Permissable<DeleteIndexRequest>` impl: permission `udex:index:v1:<name>:delete`
  - Implement `delete_index` on the `IndexServiceTrait`:
    - Extract `Claims` (return `Status::internal` if missing, same as `create_index`)
    - Validate `name` is non-empty
    - Call `self.datastore.delete_index(&name)`
    - Map `Error::IndexNotEmpty` → `Status::failed_precondition("index is not empty")`
    - Map `Error::InvalidIndex` (not found) → `Status::not_found`
    - Map other errors → `Status::internal`
- `projects/rust/server/tests/index_service_integration_tests.rs`: integration tests for happy path (empty index deleted), non-empty index (FailedPrecondition), not found (NotFound), missing name (InvalidArgument), permission denied

## Acceptance Criteria

- [x] `delete_index` handler implemented and compiles
- [x] Permission `udex:index:v1:<name>:delete` is required
- [x] Integration tests cover: happy path (empty index deleted), non-empty index (FAILED_PRECONDITION), not found (NOT_FOUND), missing name (INVALID_ARGUMENT), wrong permission (PERMISSION_DENIED)
- [x] All new integration tests pass (`cargo test -p udex-server`)

## Dependencies

- WP-01 (server trait must have `delete_index`)
- WP-02 (datastore `delete_index` + `IndexNotEmpty` error must exist)
