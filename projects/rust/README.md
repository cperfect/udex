# Udex Rust Projects
==================

This is where all the rust projects for Udex live.

These projects are:

## Udex API (./api)
Contains the generated API definitions and client/server stubs, plus any supporting artefacts, including test cases.

Depends on:
- ../protobuf

## Udex Datastore (./datastore)
Contains the datastore API definitions and mappings and migrations. Each datastore implementation (e.g. postgres, sqllite) has it's own implementation in separate packages. There will be a Cargo Feature per implementation that allows the crate to be built only for that implementation.

## Udex Core (./core)
Contains the core component including server with api implementations and the datastore mappings and migrations

Depends on:
- ./api
- ./datastore

## Udex CLI (./cli)
Contains the CLI used to run and manage Udex

Depends on:
- ./api
- ./datastore
- ./core

## Building and Running

### Prerequisites

- Rust toolchain (stable)
- Cargo
- Protobuf compiler (protoc)
- For datastore features:
  - PostgreSQL (for postgres feature)
  - (Not implemented yet) SQLite (for sqlite feature)

### Building

To build all projects:

```bash
# From the rust directory
cargo build
```

To build specific projects:

```bash
# Build API only
cargo build -p udex-api

# Build with specific datastore feature
cargo build --features "postgres"  # or "sqlite" 
```

### Running

#### Server

To run the Udex server:

```bash
# From the rust directory
cargo run -p udex-core --bin udex-server
```

#### CLI

To run the CLI:

```bash
# From the rust directory
cargo run -p udex-cli -- <command> [args]
```

Example CLI commands:
```bash
# Start the server
cargo run -p udex-cli -- server start

# Create an index
cargo run -p udex-cli -- index create my-index

# List all indices
cargo run -p udex-cli -- index list
```

### Development

#### Initialise datastore
`cd datastore && sqlx migrate run --source migrations/postgres/`

#### Running Tests

Run all tests:
```bash
cargo test
```

Run tests for specific projects:
```bash
# Test API
cargo test -p udex-api

# Test with specific datastore
cargo test -p udex-datastore --features postgres
```

#### Code Generation

The API is generated from protobuf definitions. To regenerate:

```bash
# From the rust directory
cargo build -p udex-api
```

#### Database Migrations

To run database migrations:

```bash
# From the rust directory
cargo run -p udex-datastore --bin udex-datastore-migrate -- --datastore postgres://user:pass@localhost/udex
```

### Configuration

The server and CLI can be configured using:

1. Environment variables:
```bash
UDEX_CONFIG_PATH=config.yaml cargo run -p udex-core --bin udex-server
```

2. Command line arguments:
```bash
cargo run -p udex-core --bin udex-server -- --config config.yaml
```

Example configuration files can be found in the `config` directory.
