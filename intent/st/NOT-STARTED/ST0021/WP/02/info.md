---
verblock: "21 May 2026:v0.1: Chris Perfect - Initial version"
wp_id: WP-02
title: "CLI: add 'udex health' subcommand"
scope: Small
status: Not Started
---

# WP-02: CLI — add `udex health` subcommand

## Objective

Add a `udex health` subcommand that calls the server health endpoint and prints a human-readable status, with a non-zero exit code when the server is not serving.

## Deliverables

- `projects/rust/cli/src/commands/health.rs`
- Wired into `commands/mod.rs` and `cli.rs`
- Uses SDK `health()` from WP01

## Acceptance Criteria

- [ ] `udex health` works without `--token`, OAuth2 config, or index name — only `--server` required
- [ ] Prints a clear status line: `Server is SERVING` / `Server is NOT_SERVING` / etc.
- [ ] Exits with code 0 for SERVING, non-zero otherwise (scriptable)
- [ ] Non-zero exit + meaningful message when server is unreachable

## Dependencies

- WP01 (SDK health method)
