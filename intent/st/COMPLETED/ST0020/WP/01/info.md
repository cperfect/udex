---
verblock: "21 May 2026:v0.1: Chris Perfect - Initial version"
wp_id: WP-01
title: "Proto and generated code"
scope: Small
status: Done
---

# WP-01: Proto and generated code

## Objective

Remove `hash_algorithm` from the `IndexUpdate` proto message so the type system enforces immutability at the API boundary. Regenerate the Rust types so downstream compilation errors identify every affected call site mechanically.

## Deliverables

- `projects/protobuf/udex.index.v1.proto` — `hash_algorithm` field removed from `IndexUpdate`; field number 7 reserved with a comment
- `projects/rust/api/src/generated/udex.index.v1.rs` — regenerated (via `cargo build`); `IndexUpdate` struct no longer carries `hash_algorithm`

## Acceptance Criteria

- [ ] `IndexUpdate` in the proto no longer has a `hash_algorithm` field; field number 7 is reserved
- [ ] `cargo build` regenerates the Rust types without error (compilation errors in `server/` are expected and addressed in WP02)
- [ ] No manual edits to any file under `api/src/generated/`

## Dependencies

- None; this is the first WP and unblocks WP02.
