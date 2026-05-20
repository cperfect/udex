---
verblock: "20 May 2026:v0.1: Chris Perfect - Initial version"
wp_id: WP-02
title: "Update all test fixtures to use standard health client"
scope: Medium
status: Not Started
---

# WP-02: Update all test fixtures to use standard health client

## Objective

All test fixtures that poll `HealthzServiceClient` for server readiness must switch to `tonic_health::pb::health_client::HealthClient`. No test should reference the deleted `udex.healthz.v1` types after this WP.

## Deliverables

Files to update (each polls `HealthzServiceClient` for readiness or asserts health response fields):

- `projects/rust/cli/tests/serve_live_tests.rs` -- primary readiness poll + health assertion
- `projects/rust/cli/tests/entry_live_tests.rs` -- readiness poll
- `projects/rust/cli/tests/index_oauth2_tests.rs` -- readiness poll
- `projects/rust/cli/tests/token_oauth2_tests.rs` -- readiness poll
- `projects/rust/server/tests/server_integration_tests.rs` -- `test_server_healthz` + readiness polls

In each file:
- Replace `use udex_api::healthz::{healthz_service_client::HealthzServiceClient, HealthzRequest}` with `use tonic_health::pb::{health_client::HealthClient, HealthCheckRequest}`
- Replace `.healthz(HealthzRequest {})` calls with `.check(HealthCheckRequest { service: "".into() })`
- Replace assertions on `is_healthy` / `status_messages` / `server_time` with assertions on `status == ServingStatus::Serving`
- `test_server_healthz` in `server_integration_tests.rs` needs rewriting to match the new response shape

`tonic-health` must be added as a dev-dependency in the relevant crates (`udex-cli`, `udex-server`).

## Acceptance Criteria

- [ ] No file references `udex_api::healthz` or `HealthzServiceClient` or `HealthzRequest`
- [ ] `cargo test -p udex-cli` passes (all live test fixtures reach the readiness poll successfully)
- [ ] `cargo test -p udex-server` passes (server integration tests including `test_server_healthz`)
- [ ] `cargo test -p udex-sdk` passes (sdk integration tests use shared fixture which polls healthz)

## Dependencies

- WP01 must be complete (tonic-health server side must exist before clients can connect to it)
