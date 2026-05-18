# Tasks - ST0018: Local k8s and helm development

## Work Packages

- [x] WP01 — Dockerfile for `udex-cli`
- [x] WP02 — Helm chart and k8s manifests
- [x] WP03 — Cluster management and deploy scripts
- [x] WP04 — SDK test audit: port JWT-only cases to OAuth2 fixture
- [x] WP05 — k8s test fixture (`data_k8s`)
- [x] WP06 — CI job (`k8s-test` in `01-Validation.yml`)
- [ ] WP07 — Documentation and dev-doctor updates

## Work Package Detail

### WP01 — Dockerfile

File: `projects/rust/cli/Dockerfile`

- Multi-stage: `builder` (rust:1.95.0-bookworm + protoc) and `runtime` (debian:bookworm-slim).
- `ARG PROFILE=release`; `cargo build --profile ${PROFILE} -p udex-cli`.
- Builder copies binary to `/build/udex` to normalise the `debug`/`release` path difference.
- Runtime stage: `EXPOSE 443`, `ENTRYPOINT ["udex", "serve", "--config", "/etc/udex/config.yaml"]`.
- Verify: `docker build -f projects/rust/cli/Dockerfile projects/rust/ -t udex:dev --build-arg PROFILE=dev` and `docker build ... -t udex:latest` both succeed.

### WP02 — Helm chart and k8s manifests

Directory: `projects/k8s/helm/udex/`

- `Chart.yaml` with name, version, appVersion.
- `values.yaml` with all configurable fields: image, server, authz, datastore.
- `templates/configmap.yaml` — renders the full `UdexConfig` YAML from values.
- `templates/secret.yaml` — holds `DATABASE_URL`, `tls.crt`, `tls.key` (base64-encoded from values or `--set`).
- `templates/deployment.yaml` — mounts ConfigMap as `/etc/udex/config.yaml`; mounts Secret volume for TLS files; injects `DATABASE_URL` env var from Secret.
- `templates/service.yaml` — `type: LoadBalancer`, port 443.
- Verify: `helm lint projects/k8s/helm/udex` passes with no errors.

### WP03 — Cluster management and deploy scripts

Directory: `projects/k8s/scripts/`

- `cluster-create.sh` — `k3d cluster create udex --port "8443:443@loadbalancer"`. Idempotent (skip if cluster exists).
- `cluster-delete.sh` — `k3d cluster delete udex`.
- `image-build.sh` — `docker build -f projects/rust/cli/Dockerfile projects/rust/ -t udex:latest [--build-arg PROFILE=...]`. Accepts optional `--dev` flag.
- `image-load.sh` — `k3d image import udex:latest -c udex`.
- `deploy.sh` — `helm upgrade --install udex projects/k8s/helm/udex --set ...` (passes DATABASE_URL, TLS cert/key from env or `.env`). Runs `kubectl rollout status deployment/udex`.
- `undeploy.sh` — `helm uninstall udex`.
- All scripts: `set -euo pipefail`, prerequisite checks (k3d, kubectl, helm), meaningful error messages.

### WP04 — SDK test audit: port JWT-only cases to OAuth2 fixture

File: `projects/rust/sdk/tests/integration_tests.rs`

- Review every test function that uses only the `data()` (JWT) fixture.
- For each: determine if it is testing general API behaviour or JWT-specific behaviour.
  - General API behaviour → add `data_hydra` fixture parameter and run under both.
  - JWT-specific (e.g. invalid token rejection with static key) → leave on `data()` only; add a doc comment explaining why.
- No test cases deleted; coverage only increases.

### WP05 — k8s test fixture

File: `projects/rust/sdk/tests/integration_tests.rs`

- Add `data_k8s()` fixture using `MaybeOnceAsync`.
- Reads `K8S_SERVER_URL` env var; returns early (skip) if not set.
- Connects SDK client with `ClientOptions::client_credentials(...)` against `K8S_SERVER_URL`.
- Uses the existing generated CA cert for TLS verification (same cert mounted in the pod).
- Add a representative subset of the API test cases under `data_k8s` — at minimum: healthz, create index, create entry, list entries, update index.
- Naming: `test_sdk_k8s_*` to follow the existing layer-prefix convention.

### WP06 — CI job

File: `.github/workflows/01-Validation.yml`

- New job `k8s-test`, `needs: [test]`.
- Steps: checkout → start Compose services → install k3d/kubectl/helm → generate key material → build image → create cluster → load image → deploy → rollout wait → run k8s tests → undeploy + delete cluster (always).
- `K8S_SERVER_URL: https://localhost:8443` set in job env.
- Path filter: only runs when `projects/k8s/**`, `projects/rust/cli/Dockerfile`, `projects/rust/cli/src/**`, or `projects/rust/sdk/tests/**` are touched.
- Teardown step uses `if: always()` so the cluster is deleted even on test failure.

### WP07 — Documentation and dev-doctor updates

- `projects/k8s/README.md` — local dev walkthrough: prerequisites, first-time setup, build, deploy, run tests, teardown. Include a quickstart section (5 commands to go from zero to passing k8s tests).
- `CONTRIBUTING.md` — expand the k3d/kubectl/Helm prerequisite entry; add a "Local k8s development" section cross-referencing `projects/k8s/README.md`.
- `scripts/dev-doctor.sh` — add checks for k3d (≥5.x), kubectl (≥1.x), Helm (≥4.x). Ask user before implementing: exact version or major-version-only check.
- `projects/rust/CONTRIBUTING.md` — note the `test_sdk_k8s_*` naming convention and the `K8S_SERVER_URL` skip behaviour.

## Dependencies

```text
WP01 ──────────────────────────────────────────────────┐
WP02 ──────────────────────────────────────────────────┤
WP03 (depends on WP01 + WP02 to verify deploy works) ──┤──► WP05 ──► WP06
WP04 ──────────────────────────────────────────────────┘
WP07 (can start anytime; must be complete before merge)
```

WP01-04 can be worked in parallel. WP05 needs WP01-03 runnable locally to verify the fixture against a real cluster. WP06 needs WP05 complete. WP07 is independent but must be finished before the branch is merged.
