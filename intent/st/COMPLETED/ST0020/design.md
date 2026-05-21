# Design - ST0020: Immutable Index Hash Algorithms

## Approach

Schema first: edit the proto, regenerate, then fix all compilation errors. This makes the type system do the enforcement work and surfaces every affected call site mechanically.

### WP01 — Proto and generated code

Remove `hash_algorithm` from `IndexUpdate` in `udex.index.v1.proto`. Run `cargo build` to regenerate `api/src/generated/udex.index.v1.rs`. The generated `IndexUpdate` struct will no longer carry the field; compilation errors in `server/` identify every call site that needs updating.

No field number is reused (proto wire compatibility is irrelevant for a pre-release removal, but the number should be reserved with a comment).

### WP02 — Server: init and update_index

**`ServerConfig.init_indexes`** changes from `Vec<UpdateIndexRequest>` to `Vec<CreateIndexRequest>`. `CreateIndexRequest` already carries `hash_algorithm` as a required field, which is the correct model: the algorithm is a creation-time decision.

**`IndexService::init()`** is rewritten to accept `Vec<CreateIndexRequest>`. The existing logic:

- Index does not exist: create it via `create_index_internal` (or inline the same logic).
- Index already exists: check `existing.hash_algorithm == req.hash_algorithm`; if not, return a startup error ("cannot change hash_algorithm of existing index '{}'; existing={}, configured={}"). Otherwise diff the remaining mutable fields and call `update_index_internal` if anything changed.

**`update_index` handler** removes `hash_algorithm` from the empty-field guard (`update.hash_algorithm.is_none()` check at line ~346). The handler body remains `unimplemented!()`.

**Datastore `update_index`** (not explicitly called out in WP02 originally, but part of the same mechanical cascade): `hash_algorithm` removed from the empty-field guard and from the SQL `UPDATE` query; the `$7` parameter slot is dropped and positions $8–$10 shift down to $7–$9.

### WP03 — Documentation and FAQ

**`projects/protobuf/README.md`** key design points: add a bullet stating that `Index.hash_algorithm` is immutable after creation and that changing it post-creation is rejected.

**FAQ document**: create `projects/protobuf/FAQ.md` (or append to an appropriate existing doc) with an entry:

> **Why can't I change an index's hash algorithm?**
>
> The hash algorithm determines how every context stored in the index was fingerprinted. Changing it would silently invalidate all existing hashes: lookups would miss every entry written under the previous algorithm, and concurrent writers during a change window could hash under different algorithms, producing silently divergent entries. The algorithm is therefore fixed at creation time. If you need an index with a different algorithm, delete the old index and create a new one (you will need to re-ingest all entries).

## Design Decisions

**Use `CreateIndexRequest` for `init_indexes` rather than a new bespoke type.**
`CreateIndexRequest` already expresses "I want an index with these settings including a specific hash algorithm." Reusing it avoids a new message and keeps the config type aligned with the public API. The only wrinkle is that `CreateIndexRequest` requires `hash_algorithm`; this is a feature, not a bug — the config must be explicit about the algorithm.

**Error at startup if algorithm disagrees, rather than silently ignoring.**
A silent mismatch would mean the server is running with a different algorithm than the operator expects. Failing fast at init forces the operator to reconcile the config and the database explicitly.

**Do not implement `UpdateIndex` as part of this ST.**
The handler is currently `unimplemented!()`. Making it return `INVALID_ARGUMENT` for a `hash_algorithm` field that no longer exists in the request message is a no-op — the field simply isn't there. The implementation of `UpdateIndex` is a separate concern.

## Alternatives Considered

**Keep `hash_algorithm` in `IndexUpdate` but reject it at runtime.**
Rejected: removing it from the proto is strictly better. A field that cannot legally be set should not exist in the schema. Runtime rejection is a weaker contract and leaves the door open for clients to attempt it.

**New bespoke `InitIndexRequest` type for `init_indexes`.**
Rejected: `CreateIndexRequest` already captures exactly the right semantics. A new type would be a Highlander violation.

**Soft-enforce by logging a warning instead of erroring on algorithm mismatch at init.**
Rejected: silent degradation violates No Silent Errors. The operator must fix the config.
