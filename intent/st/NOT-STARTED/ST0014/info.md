---
verblock: "16 May 2026:v0.1: vscode - Initial version"
intent_version: 2.4.0
status: Not Started
slug: explicit-permissions-only
created: 20260516
completed:
---

# ST0014: Explicit Permissions Only

## Objective

Enforce the explicit-permissions-only rule across all RPC operations: every operation must declare the exact permission scopes it requires, and no permission must imply another. `read` and `write` are independent grants; holding one does not confer the other.

## Context

A new security guideline was added to `CONTRIBUTING.md`: permissions must be explicit and non-implied. Two known violations exist in the current codebase:

1. **`LookupKeyByContextOrCreate`** — this operation both reads (looks up an existing entry) and writes (creates a new entry if absent). It currently requires only the `write` scope. It should require both `read` and `write`, or a dedicated combined scope.

2. **Bulk write operations** — `BulkWriteEntry` accepts a heterogeneous list of operations (e.g. `CreateEntry`, `LookupOrCreate`, and potentially read-like ops). Requiring a blanket `write` scope for the whole request is too coarse; the required scopes should be derived from the actual operations present in the request.

There may be further cases in the codebase that need the same audit (other RPCs, SDK helpers, CLI permission documentation).

## Related Steel Threads

- ST0013: Lookup Or Create Entry (introduced `LookupKeyByContextOrCreate` — first known case)

## Context for LLM

This document represents a single steel thread - a self-contained unit of work focused on implementing a specific piece of functionality. When working with an LLM on this steel thread, start by sharing this document to provide context about what needs to be done.

### How to update this document

1. Update the status as work progresses
2. Update related documents (design.md, impl.md, etc.) as needed
3. Mark the completion date when finished

The LLM should assist with implementation details and help maintain this document as work progresses.
