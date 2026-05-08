---
verblock: "08 May 2026:v0.1: vscode - Initial version"
wp_id: WP-06
title: "Examples"
scope: Small
status: Not Started
---

# WP-06: Examples

## Objective

Ship runnable examples under `sdk/examples/` that demonstrate the most common SDK usage patterns and compile cleanly under `cargo test --doc`.

## Deliverables

- `sdk/examples/create_entry.rs` — connect, authenticate, create an index, create an entry
- `sdk/examples/bulk_write.rs` — batch create entries
- `sdk/examples/get_entry.rs` — look up by key and by context
- Each example documents required environment variables in its header comment

## Acceptance Criteria

- [ ] `cargo build --examples -p udex-sdk` succeeds
- [ ] Examples can be run against the compose stack and produce expected output
- [ ] Doc examples in `lib.rs` compile: `cargo test --doc -p udex-sdk`

## Dependencies

- WP-04
