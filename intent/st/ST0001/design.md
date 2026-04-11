# Design - ST0001: Add structured logging

## Approach

Two-layer approach:
1. `tower_http::trace::TraceLayer::new_for_grpc()` on the tonic server for automatic request/response lifecycle logging (method, status code, latency) across all services.
2. Explicit `tracing::error!` at internal error conversion sites (`datastore_error_to_status` in entry.rs, inline in index.rs) for errors that disrupt the user's request.

## Design Decisions

- Log level is configurable via `RUST_LOG` env var, defaulting to `info`.
- Local development should set `RUST_LOG=trace`.
- JSON output format via `tracing-subscriber` with the `json` feature.
- `TraceLayer` preferred over per-handler logging to avoid duplication and ensure consistency.
- Internal errors logged once at the network boundary only; client/business errors (not found, invalid argument) are not logged as errors.

## Architecture

`udex-server::logging::init_tracing()` must be called at binary startup before `serve()`.

```
[TraceLayer]          ← logs every gRPC request/response (method, status, latency)
  [AuthnInterceptor]  ← warns on JWT failures and missing auth header
    [EntryService]    ← errors! on internal datastore failures
    [IndexService]    ← errors! on internal datastore failures
```

For tests, `udex-server::logging::init_test_tracing()` provides a human-readable subscriber
routed through `with_test_writer()`. Output is captured per-test and only visible when running:

```bash
RUST_LOG=debug cargo test -- --nocapture
```

It is called from each integration test suite's shared initialiser so all integration tests
benefit without per-test boilerplate.

## Alternatives Considered

- Per-handler `tracing::info!` for requests/responses: rejected as it duplicates boilerplate across every handler and is easy to miss on new handlers.
- `tonic_middleware::RequestInterceptor` for logging: rejected as it only sees requests, not responses or status codes.

## Out of Scope / Deferred

**`#[tracing::instrument]` on gRPC handler methods (WP-16)** — annotating handlers with `#[tracing::instrument]` would create named spans per invocation with structured fields. Deferred to the distributed tracing steel thread so that span design (field names, PII exclusions, OTLP export) is done consistently across the system rather than piecemeal.

**Concurrent log assertions in full server integration tests** — verifying log output from the shared background server task during concurrent tests requires span propagation across gRPC boundaries (e.g. via metadata headers) so that each test's events can be isolated by span ID. This is deferred to the steel thread that implements full distributed tracing with spans. At that point, a span-aware log capture layer and per-request span correlation should be implemented together.
