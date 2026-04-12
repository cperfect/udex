---
verblock: "06 Apr 2026:v0.1: vscode - Initial version\n06 Apr 2026:v0.2: vscode - Deferred to distributed tracing ST"
wp_id: WP-16
title: "Consider tracing::instrument for gRPC handler methods"
scope: Medium
status: Deferred
priority: suggestion
---

# WP-16: Consider tracing::instrument for gRPC handler methods

## Review Finding

🔵 **Suggestion** — Rather than (or in addition to) `TraceLayer`, annotating service methods with `#[tracing::instrument]` would automatically create named spans for each handler invocation, including method arguments as structured fields. This improves trace correlation and prepares the codebase for distributed tracing.

## Objective

Evaluate and implement `#[tracing::instrument]` on the gRPC service handler methods in `entry.rs` and `index.rs`.

## Approach

```rust
#[tracing::instrument(skip(self, request), fields(index_name = %req.index_name))]
async fn create_entry(
    &self,
    request: Request<CreateEntryRequest>,
) -> Result<Response<CreateEntryResponse>, Status> {
    let req = request.into_inner();
    // ...
}
```

`skip(self, request)` avoids logging the full request object (which may contain PII). Explicit `fields(...)` add the useful structured context.

## Note

This work is closely related to the distributed tracing steel thread. It may be better deferred until that work is planned, so that spans are designed consistently across the system.

## Decision

Deferred to the distributed tracing steel thread. Span design should be consistent across the system and is best done alongside OTLP export and span propagation, not in isolation.

## Acceptance Criteria

- [x] Decision made: deferred to distributed tracing ST
- [ ] If implemented: key handler methods in `entry.rs` and `index.rs` have `#[tracing::instrument]` with appropriate `skip` and `fields`
- [ ] Logs must not include PII or secrets in span fields

## Dependencies

- Distributed tracing steel thread (if deferred)
