---
verblock: "20 Apr 2026:v0.1: vscode - Initial version"
wp_id: WP-03
title: "Wire xxh3 into server and CLI as default"
scope: Small
status: Done
---

# WP-03: Wire xxh3 into server and CLI as default

## Objective

Update every call-site in the server, datastore, and CLI to use `xxh3_context_hash` and `HashAlgorithm::Xxh3`, removing all references to `Sha1`.

## Deliverables

- `projects/rust/server/src/entry.rs` — register `xxh3_context_hash` for `HashAlgorithm::Xxh3`; remove the `Sha1` arm
- `projects/rust/cli/src/commands/index.rs` — default `hash_algorithm` to `HashAlgorithm::Xxh3`
- `projects/rust/cli/src/commands/entry.rs` — replace `sha1_context_hash` with `xxh3_context_hash`
- `projects/rust/cli/src/commands/context.rs` — replace `sha1_context_hash` with `xxh3_context_hash`
- Any remaining `Sha1` / `sha1_context_hash` references across the workspace removed

## Acceptance Criteria

- [ ] No remaining references to `sha1_context_hash` or `HashAlgorithm::Sha1` in the codebase
- [ ] Server correctly registers the xxh3 hasher for xxh3 indices
- [ ] `cargo clippy --all-targets` passes with no warnings
- [ ] Integration tests pass (`cargo test`)

## Dependencies

- WP-01, WP-02
