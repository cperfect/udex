---
verblock: "15 May 2026:v0.1: vscode - Initial version"
wp_id: WP-03
title: "Server handler and bulk write support"
scope: Small
status: Not Started
---

# WP-03: Server handler and bulk write support

## Objective

Implement the `lookup_key_by_context_or_create` gRPC handler in `EntryService<D>` and extend `bulk_write_entry_operation` to handle the new `lookup_or_create` variant.

## Deliverables

- `projects/rust/server/src/entry.rs`:
  - `lookup_key_by_context_or_create` handler: validates input, recomputes the context hash from the supplied pairs, returns `INVALID_ARGUMENT` if it does not match the client-supplied `context_hash`, then calls `datastore.lookup_or_create_entry`, returns `LookupKeyByContextOrCreateResponse`.
  - `bulk_write_entry_operation` extended: handles `BulkWriteEntryOperation::LookupOrCreate` variant, maps result to `BulkWriteEntryOperationResult::LookupOrCreate`.

## Acceptance Criteria

- [ ] `cargo build -p udex-server` succeeds.
- [ ] Integration test (`test_serve_healthz_over_tls` and Hydra-backed live tests) still pass.
- [ ] Unit tests for the handler cover: found path, created path, missing index_name, missing context, hash mismatch returns INVALID_ARGUMENT.

## Dependencies

- WP-01 (proto + authz) and WP-02 (datastore) must be complete.
