---
verblock: "19 Jun 2026:v0.1: vscode - Initial version"
wp_id: WP-02
title: "Test harness: direct pod addressing"
scope: Small
status: Done
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

- [x] Two pods are discovered and forwarded to distinct local ports; both serve gRPC health (`test_sdk_k8s_multi_direct_health`)
- [x] Port-forward children are reaped on drop (verified no leftover `kubectl` processes)
- [x] `redeploy_k8s_server` returns only when total == 2 AND 2/2 Ready (fixes stale-pod/cascade)
- [x] Existing `test_sdk_k8s_*` still pass against 2 replicas (7/7 incl. smoke)

## Dependencies

- WP01 (2-replica deployment).
