---
verblock: "19 Jun 2026:v0.1: vscode - Initial version"
wp_id: WP-03
title: "Multi-instance integration tests"
scope: Small
status: Not Started
---

# WP-03: Multi-instance integration tests

## Objective

Add k8s integration tests that prove cross-instance correctness: CRUD via one instance, verify via the other.

## Deliverables

- `test_sdk_k8s_multi_*` cases in `sdk/tests/integration_tests.rs` (own ID prefix; fresh DB fixture; skipped when `K8S_SERVER_URL` unset).
- Scenarios: index visibility (Create A → List/Describe B); entry write-through A→B and B→A; delete propagation (DeleteEntry/DeleteIndex A → gone on B); LB CRUD sanity with 2 replicas.

## Acceptance Criteria

- [ ] Each multi-instance test pins requests to specific instances (port-forwarded clients), not the LB
- [ ] All `test_sdk_k8s_multi_*` pass against a 2-replica deployment
- [ ] Tests skip cleanly when `K8S_SERVER_URL` is unset (no false failures in non-k8s runs)

## Dependencies

- WP02 (port-forward harness + fixed rollout wait).
