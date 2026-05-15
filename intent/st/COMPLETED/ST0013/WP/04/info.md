---
verblock: "15 May 2026:v0.1: vscode - Initial version"
wp_id: WP-04
title: "SDK and CLI"
scope: Small
status: Done
---

# WP-04: SDK and CLI

## Objective

Expose `LookupKeyByContextOrCreate` to consumers via the Rust SDK and the CLI.

## Deliverables

- `projects/rust/sdk/src/entry.rs`: `lookup_or_create_entry(index_name, context) -> Result<LookupKeyByContextOrCreateResponse, Error>` method on `UdexClient`.
- `projects/rust/cli/src/cli.rs`: new `LookupOrCreate(EntryLookupOrCreateArgs)` variant in `EntryCommands`; `EntryLookupOrCreateArgs` struct with `--index` and `--context` args (matching the pattern of `EntryLookupArgs`).
- `projects/rust/cli/src/commands/entry.rs`: `lookup_or_create` handler printing key, context_hash, and created flag in table/json/yaml format.

## Acceptance Criteria

- [ ] `cargo build -p udex-cli` succeeds.
- [ ] `udex entry lookup-or-create --index <name> --context KEY=VALUE` works end-to-end (found and created paths).
- [ ] Output is consistent with other entry commands (table, json, yaml modes).

## Dependencies

- WP-03 (server) must be complete for end-to-end tests.
