# Implementation - ST0018: Local k8s and helm development

## As-built notes

### WP01 — Dockerfile

Multi-stage build in `projects/rust/cli/Dockerfile`. Builder stage uses `rust:1.95.0-bookworm` + protoc; runtime stage is `debian:bookworm-slim`. `ARG PROFILE=release` normalises the binary path across debug/release profiles by copying to `/build/udex`. Entrypoint: `udex serve --config /etc/udex/config.toml` (TOML, not YAML as originally spec'd).

### WP02 — Helm chart

Chart at `projects/k8s/helm/udex/`. Config rendered as `config.toml` (TOML) via ConfigMap, not YAML — the server's native format. Secrets hold `DATABASE_URL`, `tls.crt`, `tls.key` as Opaque base64. Deployment uses `imagePullPolicy: Never` (k3d local image). IngressRouteTCP (Traefik CRD) with `tls.passthrough: true` handles gRPC — standard Ingress doesn't support L4 TCP passthrough required for server-terminated TLS. Added `checksum/secret` pod annotation to trigger rolling updates when the Secret changes.

### WP03 — Scripts

All scripts in `projects/k8s/scripts/`. `cluster-create.sh` patches kubeconfig to replace `0.0.0.0` with `host.docker.internal` for devcontainer access. `deploy.sh` builds `K8S_DATABASE_URL` using `host.k3d.internal` (how k3d pods reach the Docker host network). Added `validate-k8s-test.sh` in `scripts/` to mirror the CI step locally.

### WP04 — SDK test audit

Ported all general API test cases to the Hydra fixture. Retained JWT-only test (`test_sdk_jwt_invalid_token_returns_rpc_error`) with doc comment explaining why it stays on the static fixture. Added `test_sdk_oauth2_delete_index_*` cases — these required adding `udex:index:v1:create` and `udex:index:v1:*:delete` scopes to the Hydra client registration (glob wildcard covers any index name).

### WP05 — k8s fixture

`data_k8s()` in `sdk/tests/integration_tests.rs` uses `tokio::sync::OnceCell::const_new()` (not `MaybeOnceAsync`) — OnceCell truly initialises once per process, avoiding re-runs when ref-count drops between tests. Returns `None` when `K8S_SERVER_URL` is unset; tests skip silently. `redeploy_k8s_server` polls pod count to zero before calling `wait_for_k8s_server` (healthz poll) to eliminate Traefik routing lag to a terminating pod.

### WP06 — CI job

`k8s-test` job in `01-Validation.yml` uses `dorny/paths-filter` — runs only when k8s chart, Dockerfile, CLI source, or SDK tests change. Delegates to `validate-k8s-test.sh` for parity with local dev. Teardown step uses `if: always()`.

### WP07 — Documentation

`projects/k8s/README.md` with five-command quickstart. Helm templates annotated with comments for Helm newcomers (ConfigMap vs Secret, TLS passthrough, checksum annotation, IngressRouteTCP vs Ingress). `CONTRIBUTING.md` and `projects/rust/CONTRIBUTING.md` updated with k8s section and `test_sdk_k8s_*` naming convention. `README.md` "Find Out More" table updated. `dev-doctor.sh` already had k3d/kubectl/Helm checks from prior work.

## Key decisions

- **TOML over YAML for config**: server's native config format; no conversion layer needed.
- **`imagePullPolicy: Never`**: k3d loads images directly from Docker daemon — no registry needed for local dev.
- **IngressRouteTCP + TLS passthrough**: gRPC clients verify the server certificate; Traefik must not terminate TLS. Standard Kubernetes Ingress is L7 HTTP only.
- **`tokio::sync::OnceCell` for k8s fixture**: `MaybeOnceAsync` drops data when ref-count hits zero between sequential tests; OnceCell holds data for the process lifetime.
- **Pod count polling before healthz**: `kubectl rollout status` returns when the new pod is Ready but the old pod may still be Terminating and receiving traffic via Traefik. Waiting for count ≤ 1 eliminates this race.

## Server bug fixed during WP05

`EntryService.index_hasher_fns` was only populated at server startup via `init()` — indices created via `create_index` gRPC at runtime were never registered, causing "hash function not found" on the first `create_entry`. Fixed by changing the field to `RwLock<HashMap>` with lazy DB lookup on cache miss. Regression test: `test_entry_service_create_entry_after_runtime_create_index` in `server/tests/entry_service_integration_tests.rs`.
