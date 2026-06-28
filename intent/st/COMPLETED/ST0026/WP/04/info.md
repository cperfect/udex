---
verblock: "24 Jun 2026:v0.1: vscode - Initial version"
wp_id: WP-04
title: "SDK and CLI client tracing"
scope: Small
status: Done
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

- [x] An SDK call made within a host span produces a child span and propagates the
  trace. Verified link-by-link: a hermetic SDK unit test proves the outgoing
  `traceparent` carries the ambient span's trace-id (client -> wire), and WP03
  proved the server continues an inbound `traceparent` into datastore spans (wire
  -> server -> datastore). The single live client->server->datastore trace in
  Tempo is automated in WP06.
- [x] The SDK installs no global provider (a host app's provider remains intact);
  with no provider configured the SDK spans are ordinary `tracing` spans and no
  `traceparent` is emitted (unit test `auth_interceptor_omits_traceparent_without_otel_context`).
- [x] `traceparent` is present on outgoing requests and continued by the server.
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test` pass.

## As-built notes

- **SDK = OTel API only, never a provider.** `udex-sdk` gains `opentelemetry`
  (API) + `tracing-opentelemetry` deps - not `opentelemetry_sdk`/`-otlp`. It uses
  them solely to read the ambient context and inject `traceparent`; the host app
  (or the CLI via `udex-telemetry`) owns the provider. MODULES.md note updated.
- **Client spans**: `#[tracing::instrument(name = "sdk.<op>", ...)]` on all
  entry/index wrapper methods.
- **Propagation in the existing interceptor**: `make_auth_interceptor` now also
  injects the current OTel context via the global text-map propagator into a
  `MetadataInjector` over the request metadata. No-op without a propagator/valid
  context, so non-OTel callers are unaffected.
- **CLI opt-in**: `init_cli_telemetry` initialises `udex-telemetry` as `udex-cli`
  only when `UDEX_OTLP_ENDPOINT` is set (with `UDEX_OTLP_CA`,
  `UDEX_TRACE_SAMPLE_RATIO`), and is skipped for `serve` (which inits from the
  server config) to avoid a double init. Off by default => no subscriber installed,
  preserving current CLI output (CLI test suites unchanged).
- **Docs**: crate-level rustdoc shows SDK spans nesting under a host span and the
  provider-free / no-traceparent-without-OTel behaviour (doctest).
- **Verified**: SDK injection unit tests (2), SDK lib (14) + doctests (2), SDK
  integration (37), CLI suites (all) pass; `clippy -D warnings` + `fmt` clean.

## Dependencies

- WP02 (telemetry foundation). Builds on WP03 (server-side continuation) for the
  full end-to-end trace.
