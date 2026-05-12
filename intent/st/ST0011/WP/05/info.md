---
verblock: "12 May 2026:v0.1: vscode - Initial version"
wp_id: WP-05
title: "SDK delete_index method and integration test"
scope: Small
status: Not Started
---

# WP-05: SDK delete_index method and integration test

## Objective

Add `UdexClient::delete_index` to the SDK, expose it in the README, and cover it with an integration test.

## Deliverables

- `projects/rust/sdk/src/index.rs`: add `pub async fn delete_index(&self, name: &str) -> Result<(), Error>` following the pattern of `list_indices` (call `IndexServiceClient::delete_index`, discard the empty response)
- `projects/rust/sdk/tests/integration_tests.rs`: integration test covering the happy path (create index, delete it, confirm it is gone) and the non-empty guard (create index, create an entry, attempt delete → `Error::Rpc` with `FAILED_PRECONDITION`)
- README and example updates are handled in WP-06

## Acceptance Criteria

- [ ] `UdexClient::delete_index` is callable from the SDK
- [ ] Integration tests cover: delete empty index (ok), delete non-empty index (FAILED_PRECONDITION mapped to `Error::Rpc`), confirm deleted index is gone (`NOT_FOUND` on subsequent describe)
- [ ] All new integration tests pass (`cargo test -p udex-sdk`)
- [ ] README and example updates tracked in WP-06

## Dependencies

- WP-01 (generated client must have `delete_index`)
- WP-03 (server must be implemented to run integration tests)
