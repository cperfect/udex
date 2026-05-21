---
verblock: "21 May 2026:v0.1: Chris Perfect - Initial version"
intent_version: 2.4.0
status: Completed
slug: immutable-index-hash-algorithms
created: 20260521
completed: 20260521
---

# ST0020: Immutable Index Hash Algorithms

## Objective

Remove the ability to change an index's hash algorithm after creation. The hash algorithm is a structural property of the index, not a mutable configuration field: it determines how every context hash stored in that index was computed. Allowing it to change would silently invalidate all existing hashes and introduce race conditions between concurrent writers using different algorithms.

## Context

The `IndexUpdate` message (used by `UpdateIndex` RPC) currently carries an optional `hash_algorithm` field. This was always incorrect — the field has no safe semantics post-creation:

- Any client that cached or recomputed a context hash under the old algorithm would produce lookups that miss every entry hashed under the new algorithm.
- A change window creates a race: concurrent writers may hash under different algorithms, producing silently divergent entries with no detection mechanism.
- The server's in-memory hasher cache (`index_hasher_fns` in `EntryService`) is keyed by index name and populated lazily. It has no invalidation path for algorithm changes; a comment in the code explicitly notes that algorithm mutability would require one.

We currently have only one hash algorithm (`XXH3`), so the field has never been exercisable in practice. We are pre-release and free to remove the field from the protobuf and generated code without a deprecation cycle.

Migration of an entire index to a new algorithm (a delete-and-recreate workflow) is explicitly **out of scope** for this steel thread. If migration tooling is ever needed it will be a separate steel thread.

## Scope

**In scope:**
- Remove `hash_algorithm` from the `IndexUpdate` proto message and regenerate code.
- Change `ServerConfig.init_indexes` from `Vec<UpdateIndexRequest>` to `Vec<CreateIndexRequest>`, since `hash_algorithm` is a required creation-time field and belongs on the creation request.
- Update `IndexService::init()` accordingly: use `CreateIndexRequest`; for an index that already exists, error if the configured algorithm disagrees with the stored one; otherwise update mutable fields as before.
- Remove `hash_algorithm` from the `update_index` handler's empty-field guard (the handler is currently unimplemented; this ST does not implement it).
- Update `projects/protobuf/README.md` key design points to document hash algorithm immutability.
- Add a FAQ entry explaining why the algorithm cannot be changed.

**Out of scope:**
- Implementing the `UpdateIndex` RPC body (pre-existing unimplemented stub; separate concern).
- Adding new hash algorithm variants.
- Live index migration tooling.

## Related Steel Threads

- None directly. The `or_insert` cache comment added in the server-refactor branch anticipates this change.

## Context for LLM

This document represents a single steel thread - a self-contained unit of work focused on implementing a specific piece of functionality. When working with an LLM on this steel thread, start by sharing this document to provide context about what needs to be done.

Key files to read before starting:
- `projects/protobuf/udex.index.v1.proto` — the `IndexUpdate` and `CreateIndexRequest` message definitions
- `projects/rust/server/src/index.rs` — `IndexService::init()` and `update_index` handler
- `projects/rust/server/src/config.rs` — `ServerConfig.init_indexes` field
- `projects/rust/api/src/generated/udex.index.v1.rs` — generated types (do not edit directly; change the proto and run `cargo build`)
- `projects/protobuf/README.md` — key design points section to update
- `design.md` in this ST for the approach

### How to update this document

1. Update the status as work progresses
2. Update related documents (design.md, impl.md, etc.) as needed
3. Mark the completion date when finished

The LLM should assist with implementation details and help maintain this document as work progresses.
