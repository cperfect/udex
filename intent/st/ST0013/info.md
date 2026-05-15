---
verblock: "15 May 2026:v0.1: vscode - Initial version"
intent_version: 2.4.0
status: WIP
slug: lookup-or-create-entry
created: 20260515
completed:
---

# ST0013: Lookup Or Create Entry

## Objective

Add a `LookupKeyByContextOrCreate` RPC to the entry service that atomically looks up the key for a context, and if no entry exists, creates it and returns the new key. The response indicates whether the key was found or created. The operation is treated as a write for the purposes of bulk operations and permissions.

## Context

The primary use case is Id Permanence: an Indexer may not know whether an entry exists for a given context and does not want to perform an explicit read-before-write (or a bulk-read before a bulk-write). `LookupKeyByContextOrCreate` eliminates that round trip.

Key design decisions:
- Request accepts full `ContextInput` (pairs) **and** a client-pre-computed `context_hash`. The server always recomputes the hash from the pairs; if the computed hash does not match the supplied hash it returns an `INVALID_ARGUMENT` error — even if the entry doesn't exist. This is a deliberate sanity check to catch algorithm mismatches early.
- Returns `key`, `context_hash`, and `created` flag.
- Permission: `udex:entry:v1:{index_name}:write` (since it may write).
- **Bulk**: included in `BulkWriteEntryOperation` only — NOT in `BulkReadEntryOperation`.
- As we are pre-release, there is no concern about API versioning.
- Must be implemented all the way through the stack: proto → api → server → sdk → cli, with doc updates.

## Required Changes

1. **Proto** (`projects/protobuf/udex.entry.v1.proto`): new RPC, request/response messages, add variant to `BulkWriteEntryOperation` and `BulkWriteEntryOperationResult`.
2. **API crate** (`udex-api`): regenerate from proto; add `Permissable` impl; add method to `EntryServiceAuthorizor`.
3. **Datastore crate** (`udex-datastore`): add `lookup_or_create_entry` method to `Datastore` trait returning `(Uuid, bool)` (bool = created); implement in `PostgresDatastore`.
4. **Server crate** (`udex-server`): implement handler in `EntryService<D>`; update `bulk_write_entry_operation`.
5. **SDK crate** (`udex-sdk`): add `lookup_or_create_entry` method to `UdexClient`.
6. **CLI crate** (`udex-cli`): add `entry lookup-or-create` subcommand with table/json/yaml output.
7. **Docs**: update README and FAQ.

## Related Steel Threads

- ST0012: Datastore migration control and validation (completed — provides migration infrastructure)

## Context for LLM

This document represents a single steel thread - a self-contained unit of work focused on implementing a specific piece of functionality. When working with an LLM on this steel thread, start by sharing this document to provide context about what needs to be done.

### How to update this document

1. Update the status as work progresses
2. Update related documents (design.md, impl.md, etc.) as needed
3. Mark the completion date when finished

The LLM should assist with implementation details and help maintain this document as work progresses.
