---
verblock: "06 Apr 2026:v0.1: vscode - Initial version"
wp_id: WP-17
title: "Review structured field usage consistency in log sites"
scope: Small
status: Not Started
priority: suggestion
---

# WP-17: Review structured field usage consistency in log sites

## Review Finding

🔵 **Suggestion** — The new `tracing::error!` calls correctly use structured fields (e.g. `error = %e`), which is good. The review notes this as a suggestion to confirm the pattern is applied consistently across all log sites added in this steel thread, and to establish it as the team convention going forward.

## Objective

Audit all new log sites introduced in ST0001 and confirm they use structured fields rather than string interpolation where a value is being logged.

## Convention

Prefer:
```rust
tracing::error!(error = %e, index = %name, "Failed to get index");
```

Over:
```rust
tracing::error!("Failed to get index {}: {}", name, e);
```

The `%` sigil uses `Display`, `?` uses `Debug`. Use `%` for user-visible values, `?` for internal/debug types.

## Acceptance Criteria

- [ ] All `tracing::error!`, `tracing::warn!`, `tracing::info!`, and `tracing::debug!` calls introduced in ST0001 use structured fields for any associated values
- [ ] Pattern documented in `intent/st/ST0001/impl.md` as the project logging convention

## Dependencies

- None
