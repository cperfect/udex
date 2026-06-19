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

- [ ] `test_sdk_k8s_multi_*` (own ID prefix), fresh DB fixture, skipped when `K8S_SERVER_URL` unset
- [ ] Index visibility: CreateIndex via A → List/Describe via B
- [ ] Entry write-through A→B and B→A (Create via one, Lookup via the other)
- [ ] Delete propagation: DeleteEntry/DeleteIndex via A → confirmed gone via B
- [ ] LB sanity: a CRUD round-trip via the LB endpoint with 2 replicas

### WP04 — Validation + consistency fix (if needed)

- [ ] Full loop: `cluster-create → image-load → deploy` → 2/2 Ready
- [ ] `validate-k8s-test.sh`: existing `test_sdk_k8s_*` still pass against 2 replicas
- [ ] New `test_sdk_k8s_multi_*` pass
- [ ] If a cross-instance bug surfaces: fix in `server/src` (D5) so suite is green
- [ ] `cargo fmt --check` + `cargo clippy --tests -- -D warnings` clean

### WP05 — Documentation updates

- [ ] `projects/k8s/README.md`: 2-replica default; LB round-robin across instances; direct-addressing (port-forward) test path; refresh diagram/prose if affected
- [ ] `docs/ARCHITECTURE.md`: multi-instance statelessness coverage + how cross-instance consistency is exercised
- [ ] `projects/rust/CONTRIBUTING.md`: document the `test_sdk_k8s_multi_*` convention (alongside `test_sdk_k8s_*`)
- [ ] `docs/SECRETS.md`: update only if new key material was introduced (none expected)
- [ ] Repo-wide sweep: no stale single-replica claims; all fenced code blocks have language IDs

## Task Notes

- Decisions (19 Jun 2026): direct addressing via `kubectl port-forward` (not per-pod Traefik routes); keep Deployment (not StatefulSet); fix cross-instance bugs in scope. See `design.md` D1–D5.
- Verified: server should pass cross-instance tests (index cache has datastore fallback `entry.rs:94-141`; entries uncached). No server change expected, but in scope if needed.
- Gotcha: existing `redeploy_k8s_server` waits for ≤1 pod — must change for 2 replicas (WP02).

## Dependencies

- WP02 depends on WP01 (need 2 replicas deployed).
- WP03 depends on WP02 (needs the port-forward harness + fixed rollout wait).
- WP04 depends on WP03 (runs the suite; consistency fix follows).
- WP05 (docs) depends on WP01–WP04 (documents as-built behaviour).
