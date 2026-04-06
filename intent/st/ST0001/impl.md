# Implementation - ST0001: Add structured logging

## Implementation

[Notes on implementation details, decisions, challenges, and their resolutions]

* Debug: is used to add extra context information to allow debug during testing and local development.
* Info: Requests, Responses and state changes should be info logged.
* Error: errors should be logged only once, at network boundaries and should include stacktraces back to the point of origin. Error logs should only occur for actual errors that stop or disrupt the user's request. Other kinds of errors should become warnings.

Println! usage should be replaced with structured log usage or deleted.

## Structured Field Convention

All `tracing` macro call sites must use structured fields for any associated values rather than string interpolation:

```rust
// Preferred — values as structured fields
tracing::error!(error = %e, index = %name, "Failed to get index");
tracing::warn!(error = ?err, "JWT validation error");

// Avoid — values interpolated into the message string
tracing::error!("Failed to get index {}: {}", name, e);
```

Sigil choice:
- `%` — uses `Display`; for user-visible or string-like values (addresses, names, messages)
- `?` — uses `Debug`; for internal/opaque types (error enums, complex structs)

Log sites with no associated value need no fields:

```rust
tracing::info!("Running migrations");
```

## Code Examples

[Key code snippets and examples]

## Technical Details

[Specific technical details and considerations]
**Logs MUST not include sensitive data such as secrets or PII**

## Challenges & Solutions

[Challenges encountered during implementation and how they were resolved]
