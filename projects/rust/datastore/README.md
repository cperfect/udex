# Udex Datastore

This crate provides the datastore abstraction layer for Udex, supporting multiple database backends. 

It provides an SDK library for use within udex which includes the types and traits used by core and the cli plus the implementations based on the different cargo features. The implementation includes the sqlx database migrations (which are in sql) for the implementations. 

## Features

- Abstract datastore interface
- Multiple backend implementations:
  - PostgreSQL (implemented)
  - _(Deferred)_ SQLite and other backends
- Database migrations
- Transaction support
- Connection pooling

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
udex-datastore = { path = "../datastore" }
```

## Data Model

Two tables make up the schema. `"index"` holds policy configuration for a named index; `entry_context` is a merged table that stores both the UUID entry key and its content-addressed context in a single row. The composite `UNIQUE(index_name, context_hash)` constraint enforces a strict 1:1 mapping: one context fingerprint maps to exactly one entry key **within a given index**. The same context hash may exist in different indexes and will produce independent keys.

`create_entry` is idempotent on duplicate context: if an entry already exists for the submitted `context_hash`, the existing key is returned and no new row is written.

### Tables

**`"index"`** — Named index definitions. Each index declares the operational limits and hashing algorithm applied to entries stored under it.

| Column | Type | Notes |
|---|---|---|
| `name` | `TEXT` | Primary key — identifier; Unicode letters, digits, hyphens, underscores |
| `display_name` | `TEXT` | Short human-readable label for UI use |
| `description` | `TEXT` | Human-readable description |
| `max_bulk_operations` | `INTEGER` | Maximum operations per bulk request |
| `max_key_length` | `INTEGER` | Maximum entry key length (bytes) |
| `max_value_length` | `INTEGER` | Maximum context value length (bytes) |
| `max_kv_pairs_per_context` | `INTEGER` | Maximum key-value pairs per context |
| `hash_algorithm` | `TEXT` | Algorithm used to hash contexts (e.g. `Xxh3`) |
| `created_at` | `TIMESTAMPTZ` | Set on insert |
| `created_by` | `TEXT` | Subject of the creating request |
| `updated_at` | `TIMESTAMPTZ` | Set on update; null until first update |
| `updated_by` | `TEXT` | Subject of the last updating request |

**`entry_context`** — A server-generated UUID key paired with its content-addressed context. `UNIQUE(index_name, context_hash)` enforces the 1:1 contract at the database level — uniqueness is scoped per index, not system-wide.

| Column | Type | Notes |
|---|---|---|
| `key` | `UUID` | Primary key — server-generated |
| `index_name` | `TEXT` | Foreign key → `"index".name` |
| `context_hash` | `TEXT` | Hash of `pairs`; unique per `index_name` — one context, one key within an index |
| `pairs` | `JSONB` | Versioned envelope `{ "app_version": "<semver>", "pairs": [KeyValuePair…] }`. `app_version` is the `udex-datastore` crate version at write time — a version marker for future migration authors, not a guarantee of a breaking schema change. Never exposed outside the datastore crate. |
| `hash_algorithm` | `TEXT` | Algorithm used to compute `context_hash` |

### Indexes

| Name | Columns | Purpose |
|---|---|---|
| `uq_entry_context_index_hash` | `entry_context(index_name, context_hash)` | Unique constraint + B-tree for index-scoped context lookups |

### Diagram

```mermaid
erDiagram
    index {
        text name PK
        text display_name
        text description
        integer max_bulk_operations
        integer max_key_length
        integer max_value_length
        integer max_kv_pairs_per_context
        text hash_algorithm
        timestamptz created_at
        text created_by
        timestamptz updated_at
        text updated_by
    }

    entry_context {
        uuid key PK
        text index_name FK
        text context_hash "UNIQUE per index_name"
        jsonb pairs "{ app_version, pairs: KeyValuePair[] } — internal envelope only"
        text hash_algorithm
    }

    index ||--o{ entry_context : "scopes"
```

## Development

### Running Tests

#### Unit Tests
Run unit tests:
```bash
cargo test --lib
```

#### Integration Tests
Integration tests require a running PostgreSQL instance. The tests will automatically create an isolated test database with a unique name.

**Setup:**
1. Ensure you have a PostgreSQL instance running (locally or via Docker)
2. Set the `DATABASE_URL` environment variable to point to your PostgreSQL instance

**Option A: Use Docker to start PostgreSQL:**
```bash
docker run -d \
  --name postgres-test \
  -e POSTGRES_PASSWORD=postgres \
  -p 5432:5432 \
  postgres:16-alpine

export DATABASE_URL="postgres://postgres:postgres@localhost:5432/postgres"
```

**Option B: Use existing PostgreSQL instance:**
```bash
export DATABASE_URL="postgres://username:password@localhost:5432/postgres"
```

**Run integration tests:**
```bash
cargo test --test postgres_integration_tests
```

**How it works:**
- The test framework reads `DATABASE_URL` and connects to your existing PostgreSQL instance
- It creates a new database named `udex_datastore_integration_test_<unique-id>`
- Migrations are automatically applied to the test database
- All tests run against this isolated database
- The test database is automatically cleaned up when the test binary exits

**Preserving test database for inspection:**
Set the `KEEP_FIXTURES` environment variable to keep the test database after tests:
```bash
KEEP_FIXTURES=true cargo test --test postgres_integration_tests
```

**Manual cleanup:**
To manually clean up any remaining test databases:
```bash
# List test databases
psql $DATABASE_URL -c "SELECT datname FROM pg_database WHERE datname LIKE 'udex_datastore_integration_test_%';"

# Drop a specific test database
psql $DATABASE_URL -c "DROP DATABASE udex_datastore_integration_test_<id>;"
```

### Migrations
Migrations are .sql files in the directories `./migrations/<db_flavour>. 

Migrations are managed using `sqlx` manually. To run migrations manually:

```bash
sqlx database reset -f --source  migrations/postgres
```
using the DATABASE_URL env var

Only integration test perform automatic migrations.

Datastores should check that there dbs are on the right migrations during init, and throw an error otherwise.

## Architecture

The datastore crate is organized into:

- `src/lib.rs`: Core interface and common types
- `src/postgres/`: PostgreSQL implementation
- `migrations/`: Database migration files
- `tests/`: Integration tests 

## Coding Guidelines
SQL statements should be formatted with indents one parameter or column per line for readability.