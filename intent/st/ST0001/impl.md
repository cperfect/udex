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

Production subscriber (JSON, called once at binary startup):

```rust
// src/logging.rs
pub fn init_tracing() {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = fmt().json().with_env_filter(env_filter).try_init();
}
```

Test subscriber (human-readable, respects `--nocapture`):

```rust
pub fn init_test_tracing() {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = fmt().with_env_filter(env_filter).with_test_writer().try_init();
}
```

Structured field usage at a log site:

```rust
tracing::error!(error = %e, index = %name, "Failed to get index");
tracing::warn!(error = ?err, "JWT validation error");
```

## Technical Details

**Logs MUST not include sensitive data such as secrets or PII.**

Both `init_tracing()` and `init_test_tracing()` use `try_init()` and discard the error,
making them safe to call multiple times (idempotent). The global subscriber is set at most once.

`with_test_writer()` routes output through Rust's test capture machinery. Without it, tracing
output goes to stderr and is not associated with the capturing test, so it always appears
regardless of `--nocapture`.

## Challenges & Solutions

**Duplicate JWT warn test** — a test was found that duplicated an existing warn-level assertion.
Replaced with a genuine wrong-issuer test to improve coverage without redundancy (WP-14).

**Dead `or_else` in `validate_jwt`** — a dead code path was removed after analysis confirmed
it could never be reached (WP-15).

**Log capture in full server integration tests** — verifying log output from the shared
background server task during concurrent tests requires per-request span correlation across
gRPC boundaries. Deferred to the distributed tracing steel thread (see design.md).
