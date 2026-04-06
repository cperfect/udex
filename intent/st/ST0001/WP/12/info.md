---
verblock: "06 Apr 2026:v0.1: vscode - Initial version"
wp_id: WP-12
title: "Remove or document unused opentelemetry dependencies"
scope: Small
status: Not Started
priority: minor
---

# WP-12: Remove or document unused opentelemetry dependencies

## Review Finding

🟡 **Minor** — `server/Cargo.toml` lists `opentelemetry`, `opentelemetry-otlp`, and `opentelemetry_sdk` as production dependencies. These do not appear to be used by any code in the server crate. Unused dependencies increase compile times and binary size.

## Objective

Either remove the unused opentelemetry crates, or add a comment explaining they are intentional placeholders for a future steel thread (e.g. distributed tracing).

## Options

1. **Remove them** — if there is no near-term plan to use them
2. **Retain with comment** — if they are intentional placeholders:
   ```toml
   # Reserved for the distributed tracing steel thread
   opentelemetry = "0.29.1"
   opentelemetry-otlp = "0.29.0"
   opentelemetry_sdk = "0.29.0"
   ```

## Acceptance Criteria

- [ ] Either the deps are removed and `cargo build` passes, or they have a comment explaining their purpose
- [ ] Decision is recorded here (update this WP with the outcome)

## Dependencies

- None
