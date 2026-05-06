---
verblock: "06 May 2026:v0.1: vscode - Initial version"
wp_id: WP-02
title: "Update proto/API definitions for 1:1 semantics"
scope: Small
status: Done
---

# WP-02: Update proto/API definitions for 1:1 semantics

## Objective

Update the protobuf service definition and generated Rust types to reflect the 1:1 API contract: `lookup_keys_by_context` returns a single optional key rather than a list, and `create_entry` is documented as idempotent on duplicate context (returns existing key).

## Deliverables

- Updated `.proto` file: `LookupKeysByContextResponse` carries a single optional key field instead of `repeated`
- Regenerated protobuf Rust code
- Updated `udex-api` authz/scope definitions if any reference the old list shape

## Acceptance Criteria

- [ ] `LookupKeysByContextResponse` proto message contains a single optional key (not `repeated`)
- [ ] `create_entry` RPC documentation updated to state idempotent behaviour on duplicate context
- [ ] `cargo build -p udex-api` passes with no warnings
- [ ] Proto changes are schema-first (`.proto` edited before any Rust implementation changes)

## Dependencies

- None — API definitions are the starting point; implementation WPs (WP-04, WP-05) depend on this
