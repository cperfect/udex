---
verblock: "16 May 2026:v0.1: vscode - Initial version"
wp_id: WP-03
title: "Reject empty bulk operations — validation and authz"
scope: Small
status: Done
---

# WP-03: Reject empty bulk operations — validation and authz

## Objective

An empty `BulkWriteEntryOperation` or `BulkReadEntryOperation` request (zero operations)
is meaningless and must be rejected as `InvalidArgument`. The server handlers already do
this, but after WP-02 introduces per-operation permission derivation, an empty bulk write
request would produce an empty `required_permissions()` list — causing the authz layer to
pass it through silently before the handler rejects it. Close that gap by treating empty
operations as a permission-layer error, not just a handler-layer error.

**Current state:**
- `server/src/entry.rs` bulk write handler: `operations.is_empty()` → `InvalidArgument` ✓
- `server/src/entry.rs` bulk read handler: `operations.is_empty()` → `InvalidArgument` ✓
- Authz layer: empty `required_permissions()` → permissive pass-through (gap after WP-02)

## Deliverables

- `api/src/authz/entry.rs` — `Permissable<BulkWriteEntryOperationRequest>`: if
  `self.operations` is empty return a sentinel impossible permission (e.g.
  `udex:entry:v1:{index}:__empty_bulk__`) so `is_permitted()` always denies it, OR
  extend the `Permissable` trait to return `Result<Vec<String>, tonic::Status>` and
  propagate `InvalidArgument` directly
- `api/src/authz/entry.rs` — same treatment for `Permissable<BulkReadEntryOperationRequest>`
- Unit tests in `api/src/authz/entry.rs` asserting empty bulk write and read are denied
  at the authz layer
- Integration tests in `server/tests/entry_service_integration_tests.rs` asserting that
  empty bulk write and read return `InvalidArgument` from the full stack

## Acceptance Criteria

- [ ] Empty `BulkWriteEntryOperationRequest` returns `InvalidArgument` — rejected before
  reaching the handler (authz layer, not just handler validation)
- [ ] Empty `BulkReadEntryOperationRequest` returns `InvalidArgument` — same
- [ ] Non-empty bulk requests are unaffected
- [ ] `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test` all pass

## Dependencies

- WP-02 must be complete first (the authz gap only exists after per-operation derivation
  is introduced)
