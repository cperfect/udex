---
verblock: "08 May 2026:v0.1: vscode - Initial version"
wp_id: WP-04
title: "Entry and index service wrappers"
scope: Small
status: Not Started
---

# WP-04: Entry and index service wrappers

## Objective

Expose idiomatic Rust methods on `UdexClient` for every entry and index RPC, hiding proto message construction behind domain-typed parameters and return values.

## Deliverables

- `UdexClient` methods for all entry operations: `create_entry`, `get_entry_by_key`, `get_entry_by_context`, `delete_entry`, `bulk_write`, `bulk_read`
- `UdexClient` methods for all index operations: `create_index`, `get_index`, `list_indexes`, `delete_index`
- Domain types (re-exported or newtyped from `udex-api`) used in signatures — no raw proto messages in public API
- Full rustdoc on every public method with examples

## Acceptance Criteria

- [ ] All RPC wrapper methods compile and are reachable from outside the crate
- [ ] `cargo test --doc -p udex-sdk` passes (doc examples compile)
- [ ] No raw tonic `Request`/`Response` types appear in public signatures

## Dependencies

- WP-03
