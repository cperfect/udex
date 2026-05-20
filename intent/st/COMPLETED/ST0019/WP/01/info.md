---
verblock: "20 May 2026:v0.1: Chris Perfect - Initial version"
wp_id: WP-01
title: "Add tonic-health, wire reporter, remove custom healthz"
scope: Medium
status: Done
---

# WP-01: Add tonic-health, wire reporter, remove custom healthz

## Objective

Replace the custom `HealthzService` / `HealthCheck` trait / `udex.healthz.v1` proto with `tonic-health`. Wire a `HealthReporter` into `EntryService` and `IndexService` so they can transition their status. Remove all custom healthz artefacts.

## Deliverables

- `tonic-health` added to workspace `Cargo.toml` (server dep; client in test dev-deps)
- `HealthReporter` created at server startup in `server.rs`; passed to `EntryService::new` and `IndexService::new`
- `EntryService` and `IndexService` hold a `HealthReporter` and call `set_serving`/`set_not_serving` when the datastore status changes
- Services registered with the reporter: `""`, `"udex.entry.v1.EntryService"`, `"udex.index.v1.IndexService"`
- `HealthServiceServer` (from `tonic-health`) added to the tonic router alongside entry and index
- `server/src/healthz.rs` deleted
- `HealthCheck` trait removed from `server/src/lib.rs`
- `HealthzService` removed from `server/src/lib.rs` pub exports and `server/src/server.rs`
- `projects/protobuf/udex.healthz.v1.proto` deleted
- `projects/rust/api/src/generated/udex.healthz.v1.rs` deleted (after regenerating)
- `api/build.rs` updated to remove healthz proto from compile list
- `api/src/lib.rs`/`api/src/generated/mod.rs` updated to remove healthz module
- `cargo fmt --check`, `cargo clippy`, `cargo build` all pass

## Acceptance Criteria

- [ ] `cargo build --all-targets` succeeds with no errors or new warnings
- [ ] `cargo test -p udex-server` passes (unit tests in `healthz.rs` are deleted with the file; existing mock-based tests in other modules unaffected)
- [ ] The standard health service responds on the running server: `grpc.health.v1.Health/Check` for `""` returns `SERVING`

## Dependencies

- None (first WP in the thread)
