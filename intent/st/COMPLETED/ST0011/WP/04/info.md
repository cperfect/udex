---
verblock: "12 May 2026:v0.1: vscode - Initial version"
wp_id: WP-04
title: "CLI delete command"
scope: Small
status: Done
---

# WP-04: CLI delete command

## Objective

Wire up and unhide the `udex index delete` CLI subcommand to call the new `DeleteIndex` RPC.

## Deliverables

- `projects/rust/cli/src/commands/index.rs`:
  - Remove `anyhow::bail!` stub from the `delete` function
  - Call `client.delete_index(DeleteIndexRequest { name })` via the SDK / raw client
  - Remove the `#[command(hide = true)]` attribute (or equivalent) so the command appears in `--help`
  - Map `FAILED_PRECONDITION` → exit code 4 (invalid input) with a clear message
  - Map `NOT_FOUND` → exit code 2 with a clear message
- README updates are handled in WP-06

## Acceptance Criteria

- [x] `udex index delete my-index` calls the server and prints confirmation
- [x] `udex index delete` appears in `udex index --help`
- [x] Integration tests (in `cli/tests/` using live server + Hydra, following `test_hydra_*` pattern) cover: happy path (empty index deleted), non-empty index (exit code 4), non-existent index (exit code 2)
- [x] All new integration tests pass (`cargo test -p udex-cli`)
- [x] README updates tracked in WP-06

## Dependencies

- WP-05 (CLI calls `UdexClient::delete_index` from the SDK)
- WP-03 (server must be implemented before CLI can be manually tested)
