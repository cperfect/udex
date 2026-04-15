---
verblock: "06 Apr 2026:v0.1: vscode - Initial version"
wp_id: WP-15
title: "Remove dead or_else in validate_jwt"
scope: Small
status: Done
priority: minor
---

# WP-15: Remove dead or_else in validate_jwt

## Review Finding

🟡 **Minor** (pre-existing) — In `server/src/authn.rs`, the `validate_jwt` method contains a redundant `.or_else()`:

```rust
let claims = match decode::<Claims>(token, &self.public_key, &validation) {
    Ok(token_data) => Ok(token_data.claims),
    Err(err) => {
        tracing::warn!(error = ?err, "JWT validation error");
        Err(Status::unauthenticated("Invalid JWT token"))
    }
}.or_else(|_| {
    Err(Status::unauthenticated("Failed to decode JWT token"))
})?;
```

The `.or_else()` replaces the `Err` from the match arm with a different unauthenticated error, meaning the "Invalid JWT token" message is never actually sent to clients — "Failed to decode JWT token" is always the client-visible message. This is dead code that obscures the actual error handling and is confusing for future maintainers.

## Objective

Remove the `.or_else()` so the code is honest about what the client receives.

## Fix

```rust
let claims = match decode::<Claims>(token, &self.public_key, &validation) {
    Ok(token_data) => Ok(token_data.claims),
    Err(err) => {
        tracing::warn!(error = ?err, "JWT validation error");
        Err(Status::unauthenticated("Invalid JWT token"))
    }
}?;
```

## Acceptance Criteria

- [ ] `.or_else()` removed from `validate_jwt`
- [ ] Client-facing error message is consistent with what the code actually produces
- [ ] All authn tests still pass

## Dependencies

- None — pre-existing issue, safe to fix independently
