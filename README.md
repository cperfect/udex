Udex
=====

[![CI](https://github.com/cperfect/udex/actions/workflows/01-Validation.yml/badge.svg)](https://github.com/cperfect/udex/actions/workflows/01-Validation.yml)
[![Security](https://github.com/cperfect/udex/actions/workflows/02-Security.yml/badge.svg)](https://github.com/cperfect/udex/actions/workflows/02-Security.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

> **Status**: Early development — not yet production ready. APIs and data models are subject to change.

> **Disclaimer**: Udex is provided as-is, without warranty of any kind. The author makes no guarantees regarding fitness for purpose, security, correctness, or availability. Use at your own risk.

## Overview

Udex is a universal index that maps arbitrary unique keys against contexts. It is lightweight, fast, and efficient for high transaction volumes and for managing unique entity identifiers across organisational and regulatory boundaries.

It has been built with the following integration and data management use cases in mind:

1. Providing stable and per-party keys for resolution across integration boundaries so that no parties share the same keys for the same entities (preventing compromise of one party leading to compromise of another) and that no external party needs to know the internal keys of the entity so that the internals are decoupled from the interfaces.
2. Replacing a sensitive primary key with an non-sensitive one - e.g. use a UUID rather than a Credit Card number (PAN, which is PCI-DSS restricted) as keys to interact with Credit Card accounts.
3. Attaching arbitrary metadata to an entity when the native store doesn't support it - for example data classification and leasing information.

It is not intended to be a generic entity database and aggregate queries are deliberately not supported.

For full detail on the data model, operations, components, security model, and design principles, see [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

For common questions, see the [FAQs](docs/FAQ.md).

## Core Concepts

There are four core domain concepts:

- **Index** — a named, configured namespace for entries. Indices are independent: the same context can appear in multiple indices with different keys. Index names may contain Unicode letters, digits, hyphens and underscores, and are immutable once set.
- **Context** — a set of key-value pairs that uniquely identifies an entity. Udex hashes the pairs to produce a stable **context fingerprint**. Contexts are immutable — they cannot be updated, only deleted and recreated.
- **Key** — a server-generated UUIDv4 assigned to a context within an index. Keys are globally unique across all indices and permanent for the lifetime of the entry.
- **Entry** — the binding of a key to a context within an index. The 1:1 invariant ensures that one context fingerprint maps to exactly one key within any given index (see [The 1:1 Entry–Context Model](#the-11-entrycontext-model) below).

```mermaid
classDiagram
    direction LR
    class Index {
        +String name
        +String description
        +i32 max_bulk_operations
        +HashAlgorithm hash_algorithm
    }
    class Entry {
        +UUID key
    }
    class Context {
        +String hash
    }
    class KeyValuePair {
        +String key
        +Value value
        +String? kek_id
        +String? dek
    }

    Index "1" *-- "0..*" Entry : contains
    Entry --> Context : key maps to
    Context "1" *-- "1..*" KeyValuePair : described by
```

`Value` is a union of `String`, `i64`, `f64`, and `bool`. `kek_id` and `dek` are optional envelope-encryption fields — when present they signal that the value is ciphertext and carry the wrapped key metadata. Both are opaque to the server and excluded from the context hash.

### The 1:1 Entry–Context Model

A **context** is a set of key-value pairs that uniquely describes an entity at a point in time. Udex hashes the pairs to produce a **context fingerprint**. The core invariant is:

> **One context fingerprint maps to exactly one entry key — always.**

See [Design Decisions](docs/DESIGN_DECISIONS.md#why-are-keyscontexts-11) for the reasoning behind this.

`create_entry` is idempotent: submitting the same context twice returns the same key both times. No duplicates accumulate. For the rationale behind this design see [Design Decisions](docs/DESIGN_DECISIONS.md#why-are-contexts-immutable), and for how to handle key migrations see [the FAQ](docs/FAQ.md#how-do-i-handle-key-migrations).

`lookup_key_by_context_or_create` combines the lookup and create into a single round trip: if an entry already exists for the context it is returned (`created=false`); if not, a new entry is created and returned (`created=true`). This is the recommended operation for scenarios where the Indexer cannot know in advance whether an entry exists and wants to avoid an explicit read-before-write. It requires both **read** and **write** permission and may appear in bulk write operations but not bulk read operations. See [the FAQ](docs/FAQ.md#when-should-i-use-lookup-or-create-instead-of-lookup--create) for guidance on when to use it.

### Access

The API is three gRPC services (defined in [`projects/protobuf/`](projects/protobuf/)):

| Service | Operations |
|---|---|
| `IndexService` | Create, describe, update, list, delete indices |
| `EntryService` | Create, delete, lookup by key, lookup by context, lookup-or-create, bulk read/write |
| [`grpc.health.v1.Health`](https://github.com/grpc/grpc-proto/blob/master/grpc/health/v1/health.proto) | Standard gRPC health check protocol (via `tonic-health`) |

All requests require a JWT (ES256) issued via **OAuth2 Client Credentials** flow. Permissions are scoped per index per operation — a token for one index cannot access another. The [Rust SDK](projects/rust/sdk/) and [`udex` CLI](projects/rust/cli/) are the primary clients.

### Client Usage
There are a number of roles a client can play when using Udex:
* Key Holder - uses keys to access data.
* Context Holder - has the context, but doesn't want to hand out its own keys.
* Indexer - performs indexing operations between Key Holders and Context Holders.
* Admin - maintains indices.

Combinations of the first three roles are possible: A single logical client could play both Key Holder and Context Holder roles, though generally not for the same index, and also act as an Indexer, or a client could be both the Context Holder and Indexer etc.

The Key Holder must obviously retain the key, however the Indexer has a choice - it can resolve the hash from the context (as long as it knows which hash function to apply - and it should always use the SDK for this to ensure hash stability) or it can retain the hash for re-use. The choice depends on the capabilities of the Indexer and the stability of the context data.

Admin is expected to be CI/CD or Operational role and generally would not be a Key Holder or Context Holder.

As an example of this see [Open Banking Consumer Data Right (CDR)](./docs/use_cases/AU_Open_Banking_CDR.md) use case - more specifically the [Resource Data Retrieval with Id Permanence data flow](./docs/use_cases/AU_Open_Banking_CDR.md#4a-phase-3-resource-data-retrieval-with-id-permanence-implemented-with-udex). 

## Tech Stack

| Concern | Technology |
|---|---|
| API spec | [Protobuf v3](https://protobuf.dev) — server, client, data models, and SDKs generated from `.proto` definitions via [prost](https://docs.rs/prost) / [tonic-build](https://docs.rs/tonic-build) |
| Transport | [tonic](https://docs.rs/tonic) — gRPC over HTTP/2 with TLS |
| Async runtime | [tokio](https://docs.rs/tokio) |
| TLS | [rustls](https://docs.rs/rustls) with [aws-lc-rs](https://docs.rs/aws-lc-rs) crypto backend — used for gRPC transport, datastore connections, and HTTP clients |
| Datastore | [PostgreSQL 16+](https://www.postgresql.org) accessed via [sqlx](https://docs.rs/sqlx) (compile-time verified queries, async, connection pooling) |
| Hashing | [xxhash-rust](https://docs.rs/xxhash-rust) (XXH3) — fast non-cryptographic hash used to fingerprint contexts |
| Serialization | [serde](https://docs.rs/serde) / [serde_json](https://docs.rs/serde_json) / [serde_yaml](https://docs.rs/serde_yaml) — derived on all API types; drives JSON and YAML output in the CLI |
| Authorization | OAuth2 Client Credentials flow — JWT (ES256) validated on every request via [jsonwebtoken](https://docs.rs/jsonwebtoken); permissions are scoped per index per operation |
| Logging | [tracing](https://docs.rs/tracing) + [tracing-subscriber](https://docs.rs/tracing-subscriber) (structured JSON in production, human-readable in development) |
| CLI | [clap](https://docs.rs/clap) — `udex` binary for server lifecycle, database migration management, index/entry management, JWT inspection, and context hashing |
| Error handling | [thiserror](https://docs.rs/thiserror) (library errors) + [anyhow](https://docs.rs/anyhow) (application errors) |

For the roadmap of deferred features, see the [FAQ](docs/FAQ.md#what-future-features-might-udex-support).

## License

MIT — see [LICENSE](LICENSE).

## Installation
> Placeholder will be filled in prior to release

### Deployment
> Placeholder will be filled in prior to release

### Database migrations

Udex uses [sqlx](https://docs.rs/sqlx) migrations applied automatically in the correct order. The server **never** mutates the schema without explicit permission.

#### How it works

On every startup the server checks that the database schema version matches the version expected by the running binary. If they do not match, the server **refuses to start** and logs an error with the current and expected version numbers. This prevents a code/schema mismatch from silently corrupting data.

The `apply_migrations` config option (default `false`) controls whether the server is also permitted to apply outstanding migrations before that check runs:

```yaml
datastore:
  # false (default) — server checks version but will not migrate; use `udex migrate apply` instead.
  # true  — server applies any outstanding migrations on startup, then checks.
  apply_migrations: false
```

#### Recommended workflow

Run migrations as a dedicated pre-deploy step, before starting the new server binary:

```bash
# 1. Confirm what will be applied (exits non-zero if behind)
udex migrate check --config udex.yaml

# 2. Apply outstanding migrations
udex migrate apply --config udex.yaml

# 3. Start the server (will refuse to start if schema is still wrong)
udex serve --config udex.yaml
```

Both commands read only the `datastore` section of the config file, so TLS certificate files do not need to be present when running migrations.

Setting `apply_migrations = true` is convenient for local development and CI environments where the server process is authorised to modify the schema. It is **not recommended for production** because it couples schema changes to server restarts and removes the explicit pre-deploy step.

## Development / Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) to get started. All key documents are indexed in the [Find Out More](#find-out-more) section.

## Info

This project is developed using [Claude Code](https://claude.ai/code) (Anthropic) with [Intent v2.11.x](https://github.com/matthewsinclair/intent) for steel thread and work package management. Plugins: [`rust-analyzer-lsp`](https://github.com/anthropics/claude-code-plugins).

## Find Out More

| Document | Description |
|---|---|
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Data model, operations, security model, and design principles |
| [docs/DESIGN_DECISIONS.md](docs/DESIGN_DECISIONS.md) | Rationale behind core design choices |
| [docs/FAQ.md](docs/FAQ.md) | Operational how-tos, usage guidance, and troubleshooting |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Getting started, development guidelines, and testing standards |
| [projects/rust/CONTRIBUTING.md](projects/rust/CONTRIBUTING.md) | Rust-specific coding standards and conventions |
| [SECURITY.md](SECURITY.md) | Vulnerability reporting policy |
| [.devcontainer/](.devcontainer/README.md) | VS Code dev container — tools and first-time setup |
| [projects/compose/](projects/compose/README.md) | Docker Compose — local PostgreSQL + Hydra services |
| [projects/k8s/](projects/k8s/README.md) | Helm chart and scripts for local k3d Kubernetes development |
| [projects/protobuf/](projects/protobuf/README.md) | Protobuf API definitions — source of truth for all API types |
| [projects/rust/api/](projects/rust/api/README.md) | `udex-api` — generated types, authz, hashing |
| [projects/rust/server/](projects/rust/server/README.md) | `udex-server` — gRPC server |
| [projects/rust/datastore/](projects/rust/datastore/README.md) | `udex-datastore` — PostgreSQL implementation |
| [projects/rust/sdk/](projects/rust/sdk/README.md) | `udex-sdk` — Rust client SDK |
| [projects/rust/cli/](projects/rust/cli/README.md) | `udex-cli` — command-line interface |
| [projects/rust/test-utils/](projects/rust/test-utils/README.md) | `udex-test-utils` — shared integration test fixtures (dev-only) |
