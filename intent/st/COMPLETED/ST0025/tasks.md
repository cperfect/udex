# Tasks - ST0025: Cluster integration tests

## Tasks

### WP01 — Chart: default dev to 2 replicas

- [x] Add `replicaCount: 2` to `values.yaml` (commented: dev default for multi-instance coverage)
- [x] `templates/deployment.yaml`: `replicas: {{ .Values.replicaCount }}`; update header comment (no longer "one replica")
- [x] `helm template` / `validate-lint-helm.sh` renders a 2-replica Deployment

### WP02 — Test harness: direct pod addressing

- [x] Pod discovery helper (`discover_udex_pods` via `kubectl get pods -l app.kubernetes.io/name=udex`)
- [x] `kubectl port-forward pod/<name> <localport>:443` per pod, with gRPC-health readiness poll
- [x] RAII guard that kills the port-forward child on drop (verified no leaks)
- [x] Direct hops trust server CA, SNI `localhost` (pod cert) — proven by the smoke test
- [x] Fix `redeploy_k8s_server` rollout wait: total == replicaCount AND all Ready (not ≤1) — fixes stale-pod cascade

### WP03 — Multi-instance integration tests

- [x] `test_sdk_k8s_multi_*` (own ID prefix), reuse data_k8s deployment, skipped when `K8S_SERVER_URL` unset
- [x] Index visibility: CreateIndex via A → Describe + List via B
- [x] Entry write-through A→B and B→A (Create via one, lookup_key_by_context via the other)
- [x] Delete propagation: DeleteEntry A→gone on B and B→gone on A; DeleteIndex A→gone on B
- [x] LB sanity: existing `test_sdk_k8s_*` exercise the LB path at 2 replicas (all pass)

### WP04 — Validation + consistency fix (if needed)

- [x] Full loop verified: deploy rolls out 2/2 Ready (via the fixture's redeploy)
- [x] `validate-k8s-test.sh`: existing `test_sdk_k8s_*` pass against 2 replicas
- [x] New `test_sdk_k8s_multi_*` pass (8/8 total)
- [x] No cross-instance bug surfaced → no `server/src` fix needed (validation-only)
- [x] `cargo fmt --check` + `cargo clippy --tests -- -D warnings` clean

### WP05 — Documentation updates

- [x] `projects/k8s/README.md`: 2-replica default; LB round-robin; direct-addressing (port-forward) test path; diagram + prose refreshed
- [x] `docs/ARCHITECTURE.md`: multi-instance statelessness coverage (suite-hierarchy row + rationale)
- [x] `projects/rust/CONTRIBUTING.md`: documented the `test_sdk_k8s_multi_*` convention
- [x] `docs/SECRETS.md`: no change needed (port-forward reuses existing certs; no new key material)
- [x] Repo-wide sweep: no stale single-replica claims; no new bare code fences

## Task Notes

- Decisions (19 Jun 2026): direct addressing via `kubectl port-forward` (not per-pod Traefik routes); keep Deployment (not StatefulSet); fix cross-instance bugs in scope. See `design.md` D1–D5.
- Verified: server should pass cross-instance tests (index cache has datastore fallback `entry.rs:94-141`; entries uncached). No server change expected, but in scope if needed.
- Gotcha: existing `redeploy_k8s_server` waits for ≤1 pod — must change for 2 replicas (WP02).

## Dependencies

- WP02 depends on WP01 (need 2 replicas deployed).
- WP03 depends on WP02 (needs the port-forward harness + fixed rollout wait).
- WP04 depends on WP03 (runs the suite; consistency fix follows).
- WP05 (docs) depends on WP01–WP04 (documents as-built behaviour).
