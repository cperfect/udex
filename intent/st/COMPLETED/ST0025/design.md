---
verblock: "19 Jun 2026:v0.1: vscode - Initial version"
intent_version: 2.4.0
---

# Design - ST0025: Cluster integration tests

## Approach

Raise the dev chart to a configurable replica count (default 2), give the test harness a way to address each pod directly via `kubectl port-forward`, and add SDK integration tests that CRUD through one instance and verify through another. Fix any cross-instance consistency bug the tests surface.

## Architecture

```text
                         ┌─ via LB (existing, edge cert, round-robin) ─┐
client (SDK) ──https:8443──> k3d LB ──> Traefik IngressRoute ──> Service ──> {pod-0, pod-1}

                         ┌─ direct (new, pod cert, pinned instance) ─┐
client (SDK) ──https://localhost:18443──> kubectl port-forward ──> pod-0
client (SDK) ──https://localhost:18444──> kubectl port-forward ──> pod-1
```

- LB path: unchanged. Trust = edge CA (`projects/k8s/traefik/certs/ca.crt`), SNI `host.docker.internal`.
- Direct path: bypasses Traefik, terminates against the pod's own cert. Trust = server CA (`projects/rust/server/tests/certs/ca.crt`), SNI `localhost` (pod cert SAN covers `localhost`/`127.0.0.1`).

## Design Decisions

| # | Decision | Rationale |
|---|----------|-----------|
| D1 | Configurable `replicaCount`, default **2** | Meets "default dev to 2 servers"; keeps it overridable for single-instance scenarios. |
| D2 | Direct addressing via **`kubectl port-forward` per pod** | Smallest chart change; standard way to pin to a specific pod from outside. Bypasses Traefik but that's fine — the LB path already covers the ingress. (User choice.) |
| D3 | Keep workload as a **Deployment** (not StatefulSet) | Port-forward targets pods discovered by label; stable identity isn't required. Avoids StatefulSet/headless-service/per-pod-route complexity. |
| D4 | Direct hops trust the **pod cert** (server CA, SNI `localhost`) | port-forward presents the pod's listener on localhost; the pod cert already has `localhost`/`127.0.0.1` SANs. No new certs. |
| D5 | **Fix cross-instance bugs in scope** | Per user — the suite must land green; a surfaced statefulness bug gets fixed here. |

## Cross-instance behaviour (verified)

Only per-instance state is the index→hasher cache (`entry.rs:33-38`), populated at startup from `list_indices()` (`entry.rs:54-77`). `CreateIndex` writes to the datastore but does **not** push into the cache (`index.rs:224-299`); handlers lazily fall back to the datastore on a cache miss and then populate (`entry.rs:94-141`). Entries are never cached. PostgreSQL is the single source of truth and commits before the response returns. **Expected: cross-instance CRUD succeeds without a server change.** If a gap is found (e.g. a `ListIndices` path that reads stale in-memory state), fix per D5.

## Per-file change plan

### projects/k8s — Helm chart
| File | Change |
|------|--------|
| `values.yaml` | Add `replicaCount: 2` (with a comment: dev default; multi-instance statelessness coverage). |
| `templates/deployment.yaml` | `replicas: {{ .Values.replicaCount }}`; update the header comment (no longer "one replica"). |

### projects/rust/sdk — test harness
| File | Change |
|------|--------|
| `tests/integration_tests.rs` | (a) **Pod port-forward helper**: discover pods via `kubectl get pods -l app.kubernetes.io/name=udex -o name`, spawn `kubectl port-forward pod/<name> <localport>:443` per pod, poll readiness, and wrap in an RAII guard that kills the child on drop. (b) **Fix `redeploy_k8s_server` rollout wait**: replace the "≤1 pod" loop with "exactly `replicaCount` pods Ready" (read desired count or assert ==2). (c) Per-pod SDK client builder trusting the server CA with SNI `localhost`. |

### projects/rust/sdk — tests
| File | Change |
|------|--------|
| `tests/integration_tests.rs` | New `test_sdk_k8s_multi_*` cases (own ID prefix), each pinning to specific instances. |

### projects/rust/server — only if a bug surfaces (D5)
| File | Change |
|------|--------|
| `src/entry.rs` / `src/index.rs` | Consistency fix (e.g. ensure the relevant handler consults the datastore). Not expected per the analysis above. |

### Docs (WP05)
| File | Change |
|------|--------|
| `projects/k8s/README.md` | 2-replica default; LB round-robin across instances; direct-addressing (port-forward) test path; refresh diagram/prose if affected. |
| `docs/ARCHITECTURE.md` | Multi-instance statelessness coverage and how cross-instance consistency is exercised. |
| `projects/rust/CONTRIBUTING.md` | Document the `test_sdk_k8s_multi_*` convention alongside `test_sdk_k8s_*`. |
| `docs/SECRETS.md` | Only if new key material was introduced (none expected). |

## Test scenarios (`test_sdk_k8s_multi_*`)

1. **Index visibility**: CreateIndex via instance A → ListIndices / DescribeIndex via instance B sees it.
2. **Entry write-through A→B**: CreateEntry via A → LookupEntry via B returns it.
3. **Entry write-through B→A**: CreateEntry via B → LookupEntry via A returns it.
4. **Delete propagation**: DeleteEntry via A → B confirms gone; DeleteIndex via A → B confirms gone.
5. **LB still works**: a CRUD round-trip via the LB endpoint with 2 replicas (sanity that round-robin is healthy).

All use Hydra `client_credentials` (JWT validation is stateless, so any instance accepts the token). Fresh DB per fixture (mirrors existing k8s fixture). Skipped when `K8S_SERVER_URL` is unset.

## Alternatives Considered

- **Per-pod Traefik routes + StatefulSet (rejected).** Would test the real ingress per instance but needs stable names, headless + per-pod Services, per-pod IngressRoutes, edge-cert SANs for per-pod hosts, and host resolution from the devcontainer. Too much surface for a test-addressing need. (User chose port-forward.)
- **Headless Service for in-cluster per-pod DNS + in-cluster test runner (rejected).** Avoids port-forward but requires running the test suite inside the cluster; the suite runs in the devcontainer today.

## Risks / watch-items

- **`redeploy_k8s_server` ≤1-pod wait** breaks at 2 replicas — must be fixed or the existing single-instance tests slow/hang. (In WP02.)
- **port-forward lifecycle**: zombie `kubectl` processes if the RAII guard misses a path; ensure kill-on-drop and readiness polling.
- **Default replicas=2 affects existing tests**: the current `test_sdk_k8s_*` now run against 2 pods via the LB — should still pass (stateless), but confirm.
- **Resource use**: 2 pods on k3d is fine locally; note in README.
- **Port collisions**: pick direct-forward ports (e.g. 18443/18444) distinct from 8443 and the in-process test ports.

## Validation

1. `helm template ... --set replicaCount=2` renders 2-replica Deployment; `validate-lint-helm.sh` passes.
2. `cluster-create → image-load → deploy` → 2/2 pods Ready.
3. `validate-k8s-test.sh` → existing `test_sdk_k8s_*` still pass against 2 replicas.
4. New `test_sdk_k8s_multi_*` pass (cross-instance CRUD).
5. `cargo fmt --check` + `cargo clippy --tests -- -D warnings` clean.

## Proposed work packages

- **WP01** — Chart: configurable `replicaCount` (default 2) + deployment/values.
- **WP02** — Test harness: per-pod port-forward RAII helper + pod discovery; fix `redeploy_k8s_server` for N replicas.
- **WP03** — Multi-instance `test_sdk_k8s_multi_*` cross-instance CRUD tests.
- **WP04** — End-to-end validation; fix any cross-instance consistency bug found (D5).
- **WP05** — Documentation updates (README, ARCHITECTURE, CONTRIBUTING; SECRETS if needed).
