---
verblock: "12 May 2026:v0.1: vscode - Initial version"
intent_version: 2.4.0
status: WIP
slug: implement-deleteindex-rpc-in-indexservice
created: 20260512
completed:
---

# ST0011: Implement DeleteIndex RPC in IndexService

## Objective

Implement the `DeleteIndex` RPC in `IndexService` end-to-end: proto service definition, generated Rust types, datastore layer, server handler, and CLI command. Deletion is only permitted when the index has no entries — this acts as a safety guard against deleting indices that are still in use.

## Context

The proto already defines `DeleteIndexRequest` / `DeleteIndexResponse` message types and a stub exists in the CLI, but the RPC is not wired up anywhere. The `IndexService` service definition has no `rpc DeleteIndex`, the generated server trait has no `delete_index` method, and the datastore trait has no `delete_index` operation.

The empty-index precondition (no entries) is a deliberate design choice: callers must drain all entries via the entry API before an index can be removed. This prevents accidental deletion of live indices.

Permission follows the existing pattern (`udex:index:v1:<name>:delete`).

## Scope

- Proto: add `rpc DeleteIndex` to the `IndexService` service definition
- Generated Rust: add `delete_index` to `IndexServiceClient` and the `IndexService` server trait
- Datastore trait + PostgreSQL: add `delete_index` (errors if entries exist); add `has_entries` helper or inline the check
- Server: implement the `delete_index` handler; wire up `Permissable<DeleteIndexRequest>`; full integration tests
- CLI: unhide and wire up the `delete` subcommand

## Related Steel Threads

- ST0010: Rust SDK (entry and index operations this builds on)

## Context for LLM

This document represents a single steel thread - a self-contained unit of work focused on implementing a specific piece of functionality. When working with an LLM on this steel thread, start by sharing this document to provide context about what needs to be done.

### How to update this document

1. Update the status as work progresses
2. Update related documents (design.md, impl.md, etc.) as needed
3. Mark the completion date when finished

The LLM should assist with implementation details and help maintain this document as work progresses.
