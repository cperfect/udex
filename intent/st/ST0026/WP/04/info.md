---
verblock: "24 Jun 2026:v0.1: vscode - Initial version"
wp_id: WP-04
title: "SDK and CLI client tracing"
scope: Small
status: Not Started
---

# WP-04: SDK and CLI client tracing

## Objective

Instrument the client side: the SDK emits client spans and propagates W3C
`traceparent` without owning an OTel provider (so it composes into a host app's
existing spans); the CLI - the reference client - enables full telemetry via
`udex-telemetry`.

## Deliverables

- `udex-sdk`:
  - Client spans (`#[instrument]`) around RPC calls using the SDK's existing
    `tracing` dependency.
  - A client interceptor that injects the current OTel context as W3C
    `traceparent` into outgoing gRPC metadata (composing with the existing auth
    interceptor).
  - **No global provider installation** - the SDK only uses the ambient
    `tracing`/OTel context; documented so host apps see SDK spans nested under
    their own.
- `udex-cli`:
  - Optional telemetry initialisation via `udex-telemetry` (flag/config/env),
    off by default, so CLI operations can be traced end to end against the stack.
- Example/doc snippet showing the SDK nesting under a host application's span.

## Acceptance Criteria

- [ ] An SDK call made within a host span produces a child span (verified via the
  CLI/test against Tempo): one connected trace client -> server -> datastore.
- [ ] The SDK installs no global provider (a host app's provider remains intact);
  with no provider configured the SDK spans are simply no-ops beyond `tracing`.
- [ ] `traceparent` is present on outgoing requests and continued by the server.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test` pass.

## Dependencies

- WP02 (telemetry foundation). Benefits from WP03 (server-side continuation) for
  full end-to-end traces.
