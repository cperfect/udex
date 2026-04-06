---
verblock: "06 Apr 2026:v0.1: vscode - Initial version"
wp_id: WP-06
title: "Guard init_tracing against double-initialisation panic"
scope: Small
status: Not Started
priority: major
---

# WP-06: Guard init_tracing against double-initialisation panic

## Review Finding

🟠 **Major** — `tracing_subscriber::fmt().init()` calls `set_global_default()` which panics if a global subscriber is already set. In test scenarios or if a second call site is added, this is a landmine. The project dev guide emphasises "Think of the Next Guy" and warns against leaving panics in infrastructure code.

## Objective

Make `init_tracing()` safe to call more than once.

## Recommended Fix

Use `try_init()` instead of `init()`:

```rust
pub fn init_tracing() {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let _ = fmt()
        .json()
        .with_env_filter(env_filter)
        .try_init();
}
```

The `let _ =` discards the error silently on subsequent calls — the first initialisation wins.

## Acceptance Criteria

- [ ] Calling `init_tracing()` twice does not panic
- [ ] Tests that install their own subscriber (e.g. `tracing-test`) are not broken by a prior `init_tracing()` call

## Dependencies

- Should be implemented together with or before WP-05
