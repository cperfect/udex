# Implementation - ST0020: Immutable Index Hash Algorithms

## As-built summary

Delivered in two commits on `feat/immudable-hash-algos`:

- `42b487a` — proto removal, full cascade of call-site fixes, init() rewrite (WP01+WP02 combined)
- `2f68bb7` — README update and FAQ.md creation (WP03)

WP01 and WP02 were merged into a single commit because the proto field removal and its downstream server fixes are mechanically interdependent: a partially-applied change leaves the build broken. The WP boundary remained useful for planning but not for commit structure.

## Files changed

| File | Change |
|---|---|
| `projects/protobuf/udex.index.v1.proto` | `hash_algorithm` removed from `IndexUpdate`; field 7 reserved |
| `projects/rust/api/src/generated/udex.index.v1.rs` | Regenerated — `IndexUpdate` struct no longer has the field |
| `projects/rust/datastore/src/postgres.rs` | Empty-field guard and SQL `UPDATE` stripped of `hash_algorithm`; params $8–$10 shift to $7–$9 |
| `projects/rust/datastore/tests/postgres_integration_tests.rs` | Four `IndexUpdate` literals had `hash_algorithm: None/Some(...)` removed |
| `projects/rust/server/src/config.rs` | `init_indexes` type: `Vec<UpdateIndexRequest>` → `Vec<CreateIndexRequest>` |
| `projects/rust/server/src/index.rs` | `init()` rewritten; `update_index` empty-field guard fixed |
| `projects/rust/server/tests/index_service_integration_tests.rs` | Init fixture and two `IndexUpdate` literals updated |
| `projects/rust/server/tests/server_integration_tests.rs` | Two init fixtures updated; unused imports removed |
| `projects/rust/server/tests/entry_service_integration_tests.rs` | Init fixture updated |
| `projects/rust/server/benches/common/mod.rs` | Init fixture updated |
| `projects/rust/sdk/tests/integration_tests.rs` | Two init fixtures updated; unused imports removed |
| `projects/rust/cli/tests/token_oauth2_tests.rs` | Init fixture updated; unused imports removed |
| `projects/rust/cli/tests/entry_live_tests.rs` | Init fixture updated; unused imports removed |
| `projects/rust/cli/tests/index_oauth2_tests.rs` | `index_update()` helper removed (only used for init); two inline `CreateIndexRequest` literals added |
| `projects/protobuf/README.md` | Hash algorithm immutability added to key design points |
| `projects/protobuf/FAQ.md` | New file — FAQ entry covering the three failure modes and workaround |

## Key implementation notes

**`init()` rewrite.** The old implementation used `UpdateIndexRequest` with an `IndexUpdate` sub-message where all fields were `Option<T>`, requiring explicit presence checks before constructing the `Index`. `CreateIndexRequest` carries all fields as required (non-optional) values, so the validation becomes simple range/content checks (`trim().is_empty()`, `< 1`) with no unwrap chains. The algorithm-mismatch error path is new:

```rust
if existing.hash_algorithm != req.hash_algorithm {
    return Err(Error::ServerError(format!(
        "cannot change hash_algorithm of existing index '{}': \
         stored={}, configured={}",
        req.name, existing.hash_algorithm, req.hash_algorithm
    )));
}
```

**`index_update` helper removal.** `index_oauth2_tests.rs` had a private `fn index_update(description: &str) -> IndexUpdate` helper that was only used to populate `init_indexes`. After the type change the helper was dead; it was removed and its two call sites replaced with inline `CreateIndexRequest` literals.

**Datastore parameter shift.** The SQL `UPDATE` for `update_index` previously bound ten parameters ($1–$10 including `hash_algorithm` at $7). After removal: six field parameters ($1–$6), then `updated_at` ($7), `updated_by` ($8), `name` ($9). This was verified by reading the bind chain and the SQL string together.
