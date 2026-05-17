---
verblock: "17 May 2026:v0.1: vscode - Initial version"
wp_id: WP-03
title: "Slim down entry_service and index_service integration tests"
scope: Small
status: Done
---

# WP-03: Slim down entry_service and index_service integration tests

## Objective

Reduce test bloat and runtime overhead in the service-layer integration test files by removing tests that are already covered by the SDK suite. Keep a small, focused set in each file that serves the stated purpose of the suite (validation contract for `index_service`; isolation-debugging aid for `entry_service`).

## Deliverables

### `server/tests/index_service_integration_tests.rs` — keep validation tests only

Remove tests whose scenarios are fully covered by the SDK suite and keep only those that verify input-validation logic that is hard to observe from SDK level:

**Keep:**
- `test_index_service_describe_empty_name` (was `test_describe_empty_name`)
- `test_index_service_create_unsupported_hash_algorithm` (was `test_create_index_unsupported_hash_algorithm`)
- `test_index_service_create_empty_name` (was `test_create_index_empty_name`)
- `test_index_service_create_invalid_max_bulk_operations` (was `test_create_index_invalid_max_bulk_operations`)
- `test_index_service_create_invalid_max_key_length` (was `test_create_index_invalid_max_key_length`)
- `test_index_service_create_invalid_max_value_length` (was `test_create_index_invalid_max_value_length`)
- `test_index_service_create_invalid_max_kv_pairs_per_context` (was `test_create_index_invalid_max_kv_pairs_per_context`)
- `test_index_service_create_invalid_hash_algorithm` (was `test_create_index_invalid_hash_algorithm`)
- `test_index_service_create_invalid_name_chars` (was `test_create_index_invalid_name_chars`)
- `test_index_service_create_valid_name_chars` (was `test_create_index_valid_name_chars`)
- `test_index_service_create_empty_display_name` (was `test_create_index_empty_display_name`)
- `test_index_service_create_empty_description` (was `test_create_index_empty_description`)
- `test_index_service_update_empty_name` (was `test_update_index_empty_name`)
- `test_index_service_update_missing_update` (was `test_update_index_missing_update`)

**Remove** (all covered by `test_sdk_*`):
- `test_index_server_init`
- `test_describe_valid_input`
- `test_create_index_valid_input`
- `test_create_index_duplicate_name`
- `test_update_index_valid_input`
- `test_list_indices`

### `server/tests/entry_service_integration_tests.rs` — 8 tests retained

Remove tests covered by the SDK suite. Three tests with unique service-layer coverage were restored after coverage review:

**Keep (core isolation set):**
- `test_entry_service_server_init` — verifies service-layer init path
- `test_entry_service_create_entry` — basic create in isolation
- `test_entry_service_lookup_context_by_key` — basic lookup path
- `test_entry_service_bulk_write_empty_invalid_argument` — validation
- `test_entry_service_bulk_read_empty_invalid_argument` — validation

**Restored (unique coverage not reachable via SDK):**
- `test_entry_service_error_handling` — invalid UUID format rejection; SDK always sends well-formed UUIDs
- `test_entry_service_lookup_or_create_validation_errors` — missing context / empty hash / empty index_name rejections; SDK always sends valid inputs
- `test_entry_service_lookup_or_create_hash_mismatch` — server-side hash mismatch detection; SDK always computes the correct hash

**Remove** (covered by `test_sdk_*`):
- `test_delete_entry`
- `test_lookup_key_by_context`
- `test_bulk_write_entry_operation`
- `test_bulk_read_entry_operation`
- `test_create_entry_idempotent_for_same_pairs_different_dek`
- `test_lookup_or_create_creates_new_entry`
- `test_lookup_or_create_returns_existing_entry`
- `test_bulk_write_lookup_or_create`

## Acceptance Criteria

- [x] `index_service_integration_tests.rs` contains only validation tests (no happy-path CRUD)
- [x] `entry_service_integration_tests.rs` retains 8 tests (5 core + 3 restored for unique service-layer coverage)
- [x] All remaining tests are renamed to the `test_index_service_*` / `test_entry_service_*` convention
- [x] `cargo test --all-targets` passes (all retained tests run and pass)
- [x] `cargo fmt --check`, `cargo clippy --all-targets` pass

## Dependencies

- WP-02 must be complete so fixture imports are standardised before slimming
