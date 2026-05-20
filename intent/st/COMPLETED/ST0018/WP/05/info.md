---
verblock: "18 May 2026:v0.1: vscode - Initial version"
wp_id: WP-05
title: "k8s test fixture"
scope: Small
status: Done
---

# WP-05: k8s test fixture

## Objective

Add a `data_k8s()` fixture and representative test suite to the SDK integration tests that runs against a live k3d-deployed udex server using OAuth2 auth. Tests skip silently when `K8S_SERVER_URL` is not set.

## Deliverables

- `projects/rust/sdk/tests/integration_tests.rs` — `data_k8s()` fixture and 6 `test_sdk_k8s_*` tests.

## Design notes

- `init_k8s_fixture()` returns `Option<K8sFixture>`: `None` when `K8S_SERVER_URL` is unset.
- Registers a dedicated Hydra client (`sdk-k8s-integration-test-client`) with full scopes.
- Creates the test index idempotently (handles `ALREADY_EXISTS`) since the k8s server uses a persistent datastore.
- Uses the existing test CA cert (`projects/rust/server/tests/certs/ca.crt`) for server TLS verification.

## Tests added

- `test_sdk_k8s_list_indices`
- `test_sdk_k8s_describe_index`
- `test_sdk_k8s_create_and_lookup_entry`
- `test_sdk_k8s_create_entry_idempotent`
- `test_sdk_k8s_delete_entry`
- `test_sdk_k8s_lookup_or_create_entry`

## Acceptance Criteria

- [x] Compiles cleanly with `cargo test -p udex-sdk --test integration_tests --no-run`
- [x] All tests follow `test_sdk_k8s_*` naming convention
- [x] Tests return early (no panic) when `K8S_SERVER_URL` is unset

## Dependencies

- WP01–03 (cluster + image + deploy) — needed to run the tests, not to compile them
