---
verblock: "15 May 2026:v0.1: vscode - Initial version"
wp_id: WP-01
title: "Proto schema and API layer"
scope: Small
status: Done
---

# WP-01: Proto schema and API layer

## Objective

Define the `LookupKeyByContextOrCreate` RPC in the protobuf schema, regenerate the API crate, add the authz `Permissable` implementation, and wire up the `EntryServiceAuthorizor`.

## Deliverables

- `projects/protobuf/udex.entry.v1.proto`: new RPC `LookupKeyByContextOrCreate`; new messages `LookupKeyByContextOrCreateRequest` (fields: `index_name`, `ContextInput context`, `string context_hash` — client pre-computed) and `LookupKeyByContextOrCreateResponse` (fields: `key`, `context_hash`, `created` bool); `lookup_or_create` variant added to `BulkWriteEntryOperation` and `BulkWriteEntryOperationResult` oneofs.
- API crate regenerated via `build.rs` — no hand-edits to generated code.
- `projects/rust/api/src/authz/entry.rs`: `Permissable<LookupKeyByContextOrCreateRequest>` impl with permission `udex:entry:v1:{index_name}:write`; `lookup_key_by_context_or_create` method added to `EntryServiceAuthorizor`.

## Acceptance Criteria

- [ ] `cargo build -p udex-api` succeeds with no warnings.
- [ ] `EntryServiceAuthorizor` fully implements the generated `EntryService` trait.
- [ ] Unit tests for the new `Permissable` impl cover: correct permission granted, permission denied, no claims.

## Dependencies

- None.
