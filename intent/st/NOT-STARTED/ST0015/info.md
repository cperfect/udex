---
verblock: "16 May 2026:v0.1: vscode - Initial version"
intent_version: 2.4.0
status: Not Started
slug: app-version-for-pairs-jsonb-data
created: 20260516
completed:
---

# ST0015: App Version for pairs jsonb data

## Objective

Add an `app_version` field to the JSONB envelope stored in the `entry_context.pairs` column. The field records the semantic version of the `udex-datastore` crate at the time the row was written. This gives future migration authors a version marker to detect and handle structural changes to the pairs data definition.

## Context

The `entry_context.pairs` column stores a `Vec<KeyValuePair>` serialised directly as a JSON array. If the pairs data model ever changes (fields added, renamed, or restructured), there is currently no way to tell which schema version a stored row was written with.

The fix is to wrap the array in a versioned envelope when writing to and reading from the database:

```json
{
  "app_version": "0.1.0",
  "pairs": [ { "key": "...", "value": "..." } ]
}
```

Key constraints:
- **Datastore-only concern** — the `app_version` field must never appear in API types, gRPC responses, or SDK types. It is purely an internal storage detail of the PostgreSQL implementation.
- **Pre-release, no existing installs** — we are free to update the existing migration (`01_initial_schema.sql`) in place rather than adding a second migration file. The `pairs` column type remains `JSONB`; only the content changes.
- **Version source** — use `env!("CARGO_PKG_VERSION")` to embed the version at compile time; no runtime config or injection needed.
- **Datastore docs** — `datastore/README.md` must be updated to document the new envelope structure.

## Related Steel Threads

- ST0013: Lookup Or Create Entry (introduced the upsert path that also writes `pairs`)

## Context for LLM

This document represents a single steel thread - a self-contained unit of work focused on implementing a specific piece of functionality. When working with an LLM on this steel thread, start by sharing this document to provide context about what needs to be done.

### How to update this document

1. Update the status as work progresses
2. Update related documents (design.md, impl.md, etc.) as needed
3. Mark the completion date when finished

The LLM should assist with implementation details and help maintain this document as work progresses.
