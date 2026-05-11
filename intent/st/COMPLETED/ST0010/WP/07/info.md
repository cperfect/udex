---
verblock: "08 May 2026:v0.1: vscode - Initial version"
wp_id: WP-07
title: "CLI migration to SDK"
scope: Small
status: Done
---

# WP-07: CLI migration to SDK

## Objective

Replace the CLI's inline gRPC plumbing (`cli/src/client.rs`) with `udex-sdk` so the library is validated against real CLI usage before publication.

## Deliverables

- `cli/src/client.rs` deleted; CLI commands updated to use `UdexClient` from `udex-sdk`
- `cli/Cargo.toml` gains `udex-sdk` dependency, drops now-redundant direct tonic/TLS deps
- All existing CLI commands pass their tests after migration
- MODULES.md updated to reflect that the CLI no longer owns connection/auth concerns

## Acceptance Criteria

- [ ] `cargo test -p udex-cli` passes
- [ ] CLI smoke test: `udex entry create` works against the compose stack
- [ ] No raw tonic stubs remain in `cli/src/` — all gRPC calls go through the SDK
- [ ] `cli/src/client.rs` no longer exists

## Dependencies

- WP-04
