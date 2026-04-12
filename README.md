Udex
=====

## Overview

Udex is a universal lookup directory for entities — lightweight, fast, and efficient for high transaction volumes across organisational and regulatory boundaries. It maps globally unique keys to contexts, enabling systems to integrate without exposing internal entity identifiers across boundaries.

For full detail on the data model, operations, components, security model, and design principles, see [docs/intent/ARCHITECTURE.md](docs/intent/ARCHITECTURE.md).

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

## Workspace
Udex will be developed in a git monorepo. Some kind of build tooling will be required that support polyglot projects - e.g. Nx. The entire workspaces will be used via a vscode devcontainer.
