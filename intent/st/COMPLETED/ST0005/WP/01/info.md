---
verblock: "20 Apr 2026:v0.1: vscode - Initial version"
wp_id: WP-01
title: "Add Xxh3 to HashAlgorithm proto enum"
scope: Small
status: Done
---

# WP-01: Add Xxh3 to HashAlgorithm proto enum

## Objective

Replace the `SHA1` variant in the `HashAlgorithm` protobuf enum with `XXH3` and regenerate the Rust bindings.

## Deliverables

- `projects/protobuf/udex.index.v1.proto` — replace `SHA1 = 0` with `XXH3 = 0` in `HashAlgorithm`
- Regenerated `udex-api` protobuf Rust bindings
- `as_str_name()` / `from_str_name()` round-trip verified for `XXH3`

## Acceptance Criteria

- [x] `HashAlgorithm::Xxh3` is the only algorithm variant in the generated Rust code
- [x] `as_str_name()` returns `"XXH3"` and `from_str_name("XXH3")` round-trips correctly
- [x] `cargo build` passes (compile errors in dependents are expected and fixed in WP-02/03)

## Dependencies

- None
