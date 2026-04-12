---
verblock: "06 Apr 2026:v0.1: vscode - Initial version"
wp_id: WP-11
title: "Return opaque messages to gRPC clients for internal errors"
scope: Small
status: Done
priority: major
---

# WP-11: Return opaque messages to gRPC clients for internal errors

## Review Finding

🟠 **Major** — In `entry.rs`, the internal error arms of `datastore_error_to_status()` include raw error details in the gRPC `Status::internal()` message sent to clients. For example:

```rust
DatastoreError::Database(e) => {
    tracing::error!(error = %e, "Database error");
    Status::internal(format!("Database error: {}", e))  // leaks internal details to client
}
```

OWASP secure coding practices (referenced in the project dev guide) require that internal error details are not exposed to clients. Now that the error is correctly captured in the structured log, the client-facing message should be generic.

## Objective

Replace client-facing messages for `Status::internal()` and `Status::failed_precondition()` responses with opaque strings. The full error detail remains in the log.

## Affected Arms in entry.rs

- `DatastoreError::DatabaseNotInitialized`
- `DatastoreError::NotImplemented`
- `DatastoreError::Database`
- `DatastoreError::Transaction`
- `DatastoreError::Serialization`
- `DatastoreError::DataConversion`
- `DatastoreError::Migration`

## Fix Pattern

```rust
DatastoreError::Database(e) => {
    tracing::error!(error = %e, "Database error");
    Status::internal("Internal server error")
}
```

Also review `index.rs` inline error sites for the same pattern.

## Acceptance Criteria

- [ ] No internal error detail (stack traces, db messages, field names) is present in `Status` messages returned to clients
- [ ] All affected errors are still logged in full via `tracing::error!`
- [ ] Existing tests updated to match the new opaque response messages where needed

## Dependencies

- None
