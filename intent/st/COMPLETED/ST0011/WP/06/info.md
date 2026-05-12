---
verblock: "12 May 2026:v0.1: vscode - Initial version"
wp_id: WP-06
title: "Update examples and READMEs"
scope: Small
status: Done
---

# WP-06: Update examples and READMEs

## Objective

Add a `delete_index` SDK example and ensure all READMEs accurately reflect the new operation.

## Deliverables

- `projects/rust/sdk/examples/delete_index.rs`: runnable example that connects, authenticates, and deletes the index named by `UDEX_INDEX`; prints a confirmation on success and a clear message if the index is not empty or not found
- `projects/rust/sdk/README.md`:
  - Add `delete_index` to the Index operations code block
  - Add `delete_index` row to the Examples table
  - Add usage snippet to the running-examples section
- `projects/rust/cli/README.md`: add `udex index delete` to the Index operations section
- `projects/rust/datastore/README.md`: no schema changes needed (delete is a row removal), but confirm the Data Model description is still accurate

## Acceptance Criteria

- [x] `cargo run --example delete_index` compiles and runs against a live server
- [x] SDK README Index operations section shows `delete_index`
- [x] CLI README shows `udex index delete` with correct exit code notes
- [x] No README still documents `udex index delete` as hidden or not yet available

## Dependencies

- WP-03 (server must be implemented for the example to run)
- WP-05 (SDK `delete_index` must exist before writing the example)
