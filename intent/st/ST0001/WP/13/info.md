---
verblock: "06 Apr 2026:v0.1: vscode - Initial version"
wp_id: WP-13
title: "Review RUST_LOG=debug default in devcontainer"
scope: Small
status: Not Started
priority: minor
---

# WP-13: Review RUST_LOG=debug default in devcontainer

## Review Finding

🟡 **Minor** — `.devcontainer/devcontainer.json` sets `RUST_LOG=debug` as the default environment. Combined with `TraceLayer` (which emits a span event per gRPC request), this produces very noisy output during development and integration tests. The design doc recommends `RUST_LOG=trace` for local dev but `init_tracing()` defaults to `info`.

## Objective

Choose an appropriate `RUST_LOG` default for the devcontainer that balances visibility with noise.

## Recommendation

Consider `RUST_LOG=info,udex=debug` to limit verbose output to project crates only, or `RUST_LOG=info` with a comment directing developers to set `debug`/`trace` as needed.

## Acceptance Criteria

- [ ] `RUST_LOG` default in `.devcontainer/devcontainer.json` is reviewed and set to an appropriate level with a comment explaining the choice

## Dependencies

- WP-05 (init_tracing must be called) — the devcontainer value only has effect once the subscriber is initialised
