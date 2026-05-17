---
verblock: "17 May 2026:v0.1: vscode - Initial version"
wp_id: WP-02
title: "Rename hydra to oauth2 in test names and apply consistent test_ prefix"
scope: Small
status: Not Started
---

# WP-02: Rename hydra to oauth2 in test names and apply consistent test_ prefix

## Objective

Replace duplicated fixture setup with imports from `udex-test-utils`, rename all test functions that contain `hydra` to use `oauth2` instead, and apply the layer-prefix naming convention (`test_sdk_`, `test_server_`, `test_datastore_`, `test_index_service_`, `test_entry_service_`, `test_cli_`) across every integration test file.

## Deliverables

For each file below, replace local copies of `bind_file_secret`, `hydra_*_url`, `register_hydra_client`, and `acquire_oauth2_token` with imports from `udex_test_utils`, then rename test functions to follow the convention.

- `sdk/tests/integration_tests.rs`:
  - Remove local fixture helpers; import from `udex_test_utils`.
  - Rename `test_hydra_sdk_*` → `test_sdk_oauth2_*`.
  - Rename remaining `test_sdk_*` functions that are missing the prefix (there should be none after the previous pass, but verify).

- `server/tests/server_integration_tests.rs`:
  - Remove local fixture helpers; import from `udex_test_utils`.
  - Rename `test_hydra_*` → `test_server_oauth2_*`.
  - Rename remaining server tests to `test_server_*`.

- `server/tests/index_service_integration_tests.rs`:
  - Rename all test functions to `test_index_service_*`.

- `server/tests/entry_service_integration_tests.rs`:
  - Rename all test functions to `test_entry_service_*`.

- `datastore/tests/postgres_integration_tests.rs`:
  - Rename all test functions to `test_datastore_*`.

- `cli/tests/index_hydra_tests.rs`, `cli/tests/entry_live_tests.rs`, `cli/tests/token_hydra_tests.rs`:
  - Remove local fixture helpers; import from `udex_test_utils`.
  - Rename test functions to `test_cli_*`; rename `hydra` in function names to `oauth2`.

- `projects/rust/CONTRIBUTING.md`: Update the note about `test_hydra_` prefix to reflect the new `test_*_oauth2_` convention.

## Acceptance Criteria

- [ ] No test function name contains the word `hydra` (utility functions such as `hydra_admin_url` are exempt)
- [ ] Every integration test function is prefixed with one of the canonical layer prefixes
- [ ] Fixture helper functions (`bind_file_secret`, `hydra_*_url`, `register_hydra_client`) are no longer duplicated in multiple test files
- [ ] `cargo test --all-targets` passes (all renamed tests still run and pass)
- [ ] `cargo fmt --check`, `cargo clippy --all-targets` pass

## Dependencies

- WP-01 must be complete so `udex-test-utils` exports are available
