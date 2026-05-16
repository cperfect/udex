---
verblock: "16 May 2026:v0.1: vscode - Initial version"
intent_version: 2.4.0
status: WIP
slug: index-name-constraints-and-validation
created: 20260516
completed:
---

# ST0016: Index name constraints and validation

## Objective

Enforce a character-set constraint on the index `name` field (letters, digits, hyphens, underscores — Unicode), add a mandatory `display_name` short free-text field for human-facing UI use, and make both `display_name` and `description` mandatory on creation. The `name` field stays as the primary key; `display_name` is the human-readable label for future UI surfaces.

## Context

Index names are used as primary keys and are referenced throughout the system (foreign keys, authz rules, API paths). Allowing arbitrary characters risks collisions and makes tooling harder. A constrained `name` (normalised identifier) paired with a free-text `display_name` gives the best of both: stable keys and readable labels.

As the system is pre-release with no live installs, breaking DB changes (updating the existing migration) and protobuf field renumbering are acceptable.

Key design decisions:
- `name`: Unicode letters + digits + hyphens + underscores; max length TBD by validation.
- `display_name`: short free-text (UTF-8), mandatory, mutable.
- `description`: already exists; now explicitly mandatory on creation (was effectively optional before).
- Validation lives in the server layer (gRPC handler), not the datastore.
- Proto schema is source of truth; generated Rust code is regenerated from it.

## Related Steel Threads

- ST0013 — Index operations (original CRUD implementation)
- ST0015 — App version for pairs JSONB data

## Context for LLM

This document represents a single steel thread - a self-contained unit of work focused on implementing a specific piece of functionality. When working with an LLM on this steel thread, start by sharing this document to provide context about what needs to be done.

### How to update this document

1. Update the status as work progresses
2. Update related documents (design.md, impl.md, etc.) as needed
3. Mark the completion date when finished

The LLM should assist with implementation details and help maintain this document as work progresses.
