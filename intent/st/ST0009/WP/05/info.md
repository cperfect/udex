---
verblock: "06 May 2026:v0.1: vscode - Initial version"
wp_id: WP-05
title: "Update server entry service"
scope: Small
status: Not Started
---

# WP-05: Update server entry service

## Objective

Update `udex-server`'s entry service gRPC handler and the `udex` CLI entry commands to use the new 1:1 API. The `lookup_keys_by_context` handler changes from returning a list to returning a single optional key; the CLI `lookup` command changes from iterating `resp.keys` to reading a single `resp.key`. The `create_entry` handler relies on the datastore's idempotent upsert and returns the existing key without error on duplicate context. All callers of the old list-returning path are updated.

## Deliverables

- `lookup_keys_by_context` gRPC handler updated to return single key response
- `create_entry` handler: remove any duplicate-context error path; rely on datastore idempotency
- Any other server code that destructures a `Vec<Uuid>` from lookup is updated
- CLI `entry lookup` command updated: reads `resp.key` (`Option<String>`); prints the key or a "not found" message; updates the command doc comment
- Authz scope checks and error mapping unchanged

## Acceptance Criteria

- [ ] `LookupKeysByContextResponse` is populated with a single key (or empty/not-found response) not a list
- [ ] `create_entry` called twice with identical context returns the same key in both responses, status OK
- [ ] `cargo build -p udex-server -p udex` passes with no warnings
- [ ] CLI `udex entry lookup` prints a single key when found, and a clear "not found" message when absent
- [ ] Server integration tests pass (Hydra + real database)

## Dependencies

- WP-02: Proto definitions (response message shape)
- WP-04: Datastore trait returns `Option<Uuid>` from lookup
