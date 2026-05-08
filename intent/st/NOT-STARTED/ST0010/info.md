---
verblock: "08 May 2026:v0.1: vscode - Initial version"
intent_version: 2.4.0
status: Not Started
slug: rust-sdk
created: 20260508
completed:
---

# ST0010: Rust SDK

## Objective

Deliver `udex-sdk`, a first-class Rust client library for Udex that hides
proto/gRPC boilerplate, manages the OAuth2 client-credentials token lifecycle,
and provides a clean, idiomatic Rust API surface suitable for publication on
crates.io. Migrate the existing CLI to use the SDK so the library is
immediately exercised in production.

## Context

### Background

The current CLI talks directly to the generated tonic stubs, handling token
injection, TLS setup, and channel construction inline. Any external Rust
application wanting to integrate with Udex would have to replicate this
plumbing. The SDK centralises it into a single, tested, documented crate.

The SDK should follow established patterns for Rust gRPC client libraries:
- A `UdexClient` struct instantiated from a typed `ClientOptions` / builder
- Strongly-typed methods that accept and return domain types rather than raw
  proto messages where possible
- An async, `Send + Sync` design compatible with tokio
- Token acquisition and refresh handled transparently using the OAuth2
  client-credentials flow (same as the dev container's Hydra setup)
- TLS configured from PEM/file paths or embedded bytes

### CLI dog-fooding

The CLI already has a thin `ClientConfig` / `interceptor()` abstraction in
`cli/src/client.rs`. Replacing that with the SDK validates the library against
real usage and ensures the SDK's API is ergonomic before publication.

### Publish intent

`udex-sdk` is intended for publication to crates.io. The crate must therefore
have complete doc comments on all public items, a crate-level rustdoc overview,
and examples that compile under `cargo test --doc`.

## Scope

- New `sdk/` crate `udex-sdk` added to the Rust workspace
- `UdexClient` with connection, TLS, and OAuth2 token management
- High-level async methods for all entry and index operations
- Integration tests exercising the full stack against the compose environment
- Runnable examples under `sdk/examples/`
- CLI migrated to use `udex-sdk` (removes `cli/src/client.rs`)
- MODULES.md updated

## Out of Scope

- Non-Rust SDK bindings (Python, Go, etc.)
- REST/HTTP transport (gRPC only for now)
- Token storage / keychain integration (tokens held in memory only)
- Publishing to crates.io (that is a follow-on deployment step)

## Work Packages

| WP | Title | Status |
|----|-------|--------|
| WP-01 | Crate scaffold and workspace integration | Not Started |
| WP-02 | Core client struct, TLS, and connection management | Not Started |
| WP-03 | OAuth2 client-credentials token lifecycle | Not Started |
| WP-04 | Entry and index service wrappers | Not Started |
| WP-05 | Integration tests | Not Started |
| WP-06 | Examples | Not Started |
| WP-07 | CLI migration to SDK | Not Started |

## Related Steel Threads

- ST0008: Inject keys and secrets — establishes the TLS cert/key model and
  OAuth2 client config that the SDK will consume
- ST0009: One-to-one entry-context model — defines the entry API the SDK wraps

## Context for LLM

This document represents a single steel thread - a self-contained unit of work focused on implementing a specific piece of functionality. When working with an LLM on this steel thread, start by sharing this document to provide context about what needs to be done.

### How to update this document

1. Update the status as work progresses
2. Update related documents (design.md, impl.md, etc.) as needed
3. Mark the completion date when finished

The LLM should assist with implementation details and help maintain this document as work progresses.
