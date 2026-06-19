# Implementation - ST0025: Cluster integration tests

## Implementation

### WP01 — Chart: default dev to 2 replicas (as built)

- `values.yaml`: added `replicaCount: 2` with a comment explaining the dev default (always exercise the multi-instance stateless invariant).
- `templates/deployment.yaml`: `replicas: {{ .Values.replicaCount }}`; header comment updated from "one replica" to ".Values.replicaCount copies (default 2)".

Verified (helm v4.0.0): `validate-lint-helm.sh` passes; `helm template --show-only templates/deployment.yaml` renders `replicas: 2` by default and `replicas: 3` with `--set replicaCount=3`.

### WP02 — Test harness: direct pod addressing (as built)

All in `projects/rust/sdk/tests/integration_tests.rs`:

- `K8S_REPLICAS: usize = 2` constant (matches the chart default).
- `discover_udex_pods()` — Running pods via `kubectl get pods -l app.kubernetes.io/name=udex -o jsonpath`.
- `PortForward` struct: spawns `kubectl port-forward pod/<name> <localport>:443` (std::process::Child), polls gRPC health over TLS (SNI `localhost`, server CA) until ready; `Drop` kills+reaps the child (RAII, no leaks).
- Direct hops trust the **pod cert** (server CA), SNI `localhost` (a pod-cert SAN); the LB path keeps the edge cert.
- `test_sdk_k8s_multi_direct_health` smoke test: ensures the 2-replica deploy is up (via `data_k8s()`), discovers both pods, forwards each, and asserts SERVING directly on each.

**Key fix — rollout wait.** Replaced `redeploy_k8s_server`'s old "wait until ≤1 pod" loop (correct only for 1 replica) with `udex_pod_counts()` and a "wait until total == K8S_REPLICAS AND ready == K8S_REPLICAS" condition. Why it matters: a pod left over from a prior test run points at a **dropped** test DB; if the wait proceeds while that stale pod is still in the LB rotation, the fixture's `create_index` hits it and fails with `code 13 Internal server error`. A failed `init_k8s_fixture` then leaves the `OnceCell` uninitialised, so the next test **retries the whole redeploy** → cascade of overlapping deploys (observed: 3 ReplicaSets / 6 pods / 2 DBs, all k8s tests failing). Waiting for an exact, fully-Ready fleet removes the stale pod before any request and breaks the cascade.

Verified live: `validate-k8s-test.sh` → 7/7 k8s tests pass (6 existing + smoke) against 2 replicas; no leaked `kubectl port-forward` processes after the run; `cargo fmt --check` + `cargo clippy --tests -- -D warnings` clean.

## Code Examples

[Key code snippets and examples]

## Technical Details

[Specific technical details and considerations]

## Challenges & Solutions

[Challenges encountered during implementation and how they were resolved]
