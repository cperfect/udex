---
verblock: "06 May 2026:v0.1: vscode - Initial version"
intent_version: 2.4.0
status: Not Started
slug: one-to-one-entry-context-model
created: 20260506
completed:
---

# ST0009: One-to-one entry-context model

## Objective

Refine the Udex entry API to enforce a strict one-to-one relationship between
an entry key and a context fingerprint — one context produces exactly one entry
key within a given index. This eliminates an ambiguous and unused capability
(multiple keys per identical context), makes `lookup_keys_by_context` a
deterministic point lookup, and simplifies the full stack from API contract
through server implementation to PostgreSQL schema. The data model change
(merging `entry` + `context` into a single `entry_context` table) flows from
and implements this API constraint.

## Context

### Background

The current entry API allows multiple entry keys to be created against the same
context fingerprint (identical key-value pairs). The `lookup_keys_by_context`
endpoint therefore returns a `Vec` of keys. This was an implicit design choice
rather than a deliberate product decision.

During design review we identified that many-entries-per-context has no
legitimate production use case. The only scenario raised — key rotation during
client migration — is better served by the caller adding a version discriminator
to the context pairs (e.g. `key_version: "2"`), producing a distinct fingerprint
and thus a distinct entry. Intent becomes explicit in the data rather than
implicit in entry count.

Enforcing 1:1 at the API level makes the contract unambiguous:

- `create_entry` for a context that already has an entry is idempotent —
  returns the existing key. Callers get natural at-least-once safety.
- `lookup_keys_by_context` becomes a deterministic point lookup — returns one
  key or nothing, never a list to disambiguate.
- Key rotation is modelled explicitly via versioned contexts, which is
  queryable and auditable.

### Data model consequence

The 1:1 API constraint allows collapsing the two-table `entry` + `context`
schema into a single `entry_context` table with a `UNIQUE` constraint on
`context_hash`. This eliminates the join on the hot read path, reduces the
write path from a two-phase upsert+insert to a single `INSERT … ON CONFLICT`,
and removes the conditional context GC logic on delete.

### Decisions

- API: `lookup_keys_by_context` return type changes from list to single result.
- API: `create_entry` is idempotent on duplicate context — returns existing key.
- Schema: replace `entry` + `context` with a single `entry_context` table.
- `entry_context.key UUID PRIMARY KEY` — server-generated opaque identifier.
- `entry_context.context_hash TEXT NOT NULL UNIQUE` — enforces 1:1 at DB level.
- `entry_context.index_name TEXT NOT NULL REFERENCES index(name)`.
- Context columns (`pairs`, `dek`, `kek_id`, `hash_algorithm`) stored inline.
- Old migrations are discarded; new migrations written from scratch — no
  installs to migrate, no data migration required.
- All API and implementation changes are unrestricted — no external clients.

### Benchmark strategy

Criterion baselines are captured on the current schema before any code changes,
then compared after. The `bench_create_entry` benchmark must be updated to use
a unique context per iteration (the current shared-context reuse pattern breaks
under 1:1). Both baselines use the same updated benchmark code so the
comparison is valid.

## Scope

- Update proto/API definitions to reflect 1:1 semantics
- Update `udex-datastore` schema, queries, and `Datastore` trait
- Discard and replace PostgreSQL migrations
- Update `udex-server` entry service and all code depending on list return types
- Update benchmarks: unique-context fix + before/after baseline capture
- Update `datastore/README.md` data model section and ER diagram
- Update `ARCHITECTURE.md` and any other docs referencing many-per-context

## Out of Scope

- Data migration (no installs exist)
- SQLite or other backend implementations
- Changes to the `index` table or index service

## Related Steel Threads

- ST0008: Inject keys and secrets (completed — establishes secrets model used
  by integration tests and benchmarks)

## Context for LLM

This document represents a single steel thread - a self-contained unit of work focused on implementing a specific piece of functionality. When working with an LLM on this steel thread, start by sharing this document to provide context about what needs to be done.

### How to update this document

1. Update the status as work progresses
2. Update related documents (design.md, impl.md, etc.) as needed
3. Mark the completion date when finished

The LLM should assist with implementation details and help maintain this document as work progresses.
