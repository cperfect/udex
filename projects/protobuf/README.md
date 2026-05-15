# Udex Protobuf Definitions

This directory is the **source of truth for all Udex API types and service contracts**. All server implementations, client SDKs, and CLI tooling are generated from these files — if you are changing an API, start here.

## Files

| File | Package | Service |
|---|---|---|
| `udex.index.v1.proto` | `udex.index.v1` | `IndexService` — index lifecycle management |
| `udex.entry.v1.proto` | `udex.entry.v1` | `EntryService` — context-key entry operations |
| `udex.healthz.v1.proto` | `udex.healthz.v1` | `HealthzService` — server health check |

File naming convention: `udex.<domain>.v<n>.proto`.

## Services

### IndexService (`udex.index.v1`)

Manages the index namespace. An index is a named, configured namespace for entries.

| RPC | Description |
|---|---|
| `Describe` | Return configuration and metadata for a named index |
| `CreateIndex` | Create a new index |
| `UpdateIndex` | Update mutable fields of an existing index |
| `ListIndices` | Return all indices |
| `DeleteIndex` | Delete an index — fails if it still has entries |

### EntryService (`udex.entry.v1`)

Manages context-to-key entries within an index. All write operations are transactional.

| RPC | Description |
|---|---|
| `CreateEntry` | Create an entry; idempotent — same context returns the same key |
| `DeleteEntry` | Remove an entry by key |
| `LookupContextByKey` | Resolve a key to its context |
| `LookupKeyByContext` | Resolve a context hash to its key (returns empty if not found) |
| `BulkWriteEntryOperation` | Transactional batch of create/delete operations |
| `BulkReadEntryOperation` | Batch of lookup operations |

### HealthzService (`udex.healthz.v1`)

Single `Healthz` RPC that returns server time, health status, and any status messages.

## Key design points

- **Context identity** — the server computes the context hash over `(key, value)` pairs only; `kek_id` and `dek` envelope-encryption fields on `KeyValuePair` are excluded so that re-encrypting a value does not produce a new identity.
- **Immutability** — `Context` is immutable after creation; updates require delete and recreate. `Index.name` is also immutable.
- **`CreateEntry` is idempotent** — if the context already exists in the index the existing key is returned, never a duplicate.

## Code generation

`projects/rust/api/build.rs` compiles all three files using [`tonic-build`](https://docs.rs/tonic-build) and writes the generated Rust into `projects/rust/api/src/generated/`. The build script also derives `serde::Serialize` / `serde::Deserialize` on every generated type.

To regenerate after editing a `.proto` file, run `cargo build` from `projects/rust/`.

## Adding or changing an API

1. Edit the `.proto` file here.
2. Run `cargo build` — generated code in `api/src/generated/` is updated automatically.
3. Fix any compilation errors in `api`, `server`, `sdk`, and `cli` that result from the change.
4. Update or add tests to cover the new behaviour.
