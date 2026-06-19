---
verblock: "19 Jun 2026:v0.1: vscode - Initial version"
wp_id: WP-02
title: "Test harness: direct pod addressing"
scope: Small
status: Not Started
---

# WP-02: Test harness: direct pod addressing

## Objective

Give the SDK integration-test harness a way to address each pod directly via `kubectl port-forward`, and fix the rollout wait for multi-replica deployments.

## Deliverables

- Pod discovery helper (`kubectl get pods -l app.kubernetes.io/name=udex -o name`).
- Per-pod `kubectl port-forward pod/<name> <localport>:443` spawner with readiness polling.
- RAII guard that kills the port-forward child process on drop (no zombies).
- Per-pod SDK client builder trusting the server CA with SNI `localhost`.
- `redeploy_k8s_server` rollout wait fixed: wait for exactly `replicaCount` Ready pods (not ≤1).

## Acceptance Criteria

- [ ] Two pods are discovered and forwarded to distinct local ports; both serve gRPC health
- [ ] Port-forward children are reaped on drop (verified no leftover `kubectl` processes)
- [ ] `redeploy_k8s_server` returns only when 2/2 pods are Ready
- [ ] Existing `test_sdk_k8s_*` still pass against 2 replicas

## Dependencies

- WP01 (2-replica deployment).
