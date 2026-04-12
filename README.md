Udex
=====

[![CI](https://github.com/cperfect/udex/actions/workflows/01-Validation.yml/badge.svg)](https://github.com/cperfect/udex/actions/workflows/01-Validation.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

> **Status**: Early development — not yet production ready. APIs and data models are subject to change.

## Overview

Udex is a universal lookup directory for entities — lightweight, fast, and efficient for high transaction volumes across organisational and regulatory boundaries. It maps globally unique keys to contexts, enabling systems to integrate without exposing internal entity identifiers across boundaries.

For full detail on the data model, operations, components, security model, and design principles, see [docs/intent/ARCHITECTURE.md](docs/intent/ARCHITECTURE.md).

## Getting Started

### Prerequisites

- **Rust** (stable) — install via [rustup](https://rustup.rs/)
- **Docker** — used to run a local PostgreSQL instance for integration tests
- **protoc** (Protocol Buffers compiler) — required to build the API crate from `.proto` definitions

  ```bash
  # macOS
  brew install protobuf

  # Debian/Ubuntu
  apt-get install protobuf-compiler
  ```

A [VS Code dev container](.devcontainer) is provided that installs all prerequisites automatically — this is the recommended way to get a consistent environment.

### Build & Test

```bash
# Clone the repository
git clone https://github.com/cperfect/udex.git && cd udex

# Start PostgreSQL (or use the dev container, which starts it automatically)
docker run -d -p 5432:5432 -e POSTGRES_PASSWORD=postgres postgres:16

# Build the workspace
cargo build

# Run the full test suite
DATABASE_URL=postgres://postgres:postgres@localhost:5432/postgres cargo test
```

See [projects/rust/CONTRIBUTING.md](projects/rust/CONTRIBUTING.md) for the full pre-commit checklist and local check commands.

## Contributing Guides

- [CONTRIBUTING.md](CONTRIBUTING.md) — general development principles, guidelines, and testing standards for all contributors
- [projects/rust/CONTRIBUTING.md](projects/rust/CONTRIBUTING.md) — Rust-specific coding standards, error conventions, and local check commands
- [docs/intent/ARCHITECTURE.md](docs/intent/ARCHITECTURE.md) — full architecture intent: components, operations, security model, and design principles

This project is developed using [Claude Code](https://claude.ai/code) (Anthropic) with [Intent v2.8.0](https://github.com/matthewsinclair/intent) for steel thread and work package management. Plugins: [`rust-analyzer-lsp`](https://github.com/anthropics/claude-code-plugins). Skills: [`in-essentials`](https://github.com/matthewsinclair/intent).

## Tech Stack

* **API spec**: Protobuf v3 — server, client, data models, and SDKs are generated from proto definitions.
* **Language**: Rust — server built on [tokio](https://docs.rs/tokio) with [tonic](https://docs.rs/tonic) for gRPC. _(Deferred)_ Optional REST interface via Hyper.
* **Versioning**: Udex is semantically versioned.
* _(Deferred)_ **Observability**: OpenTelemetry tracing and metrics.

## License

MIT — see [LICENSE](LICENSE).
