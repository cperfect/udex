# Design - ST0018: Local k8s and helm development

## Approach

Deliver local Kubernetes development as a thin layer on top of what already exists: the `udex` binary (which embeds the server), the Compose-managed Hydra and PostgreSQL services, and the existing SDK integration test suite. No new server code is written; the k8s artefacts configure and deploy the binary that already exists.

Work proceeds in this order:

1. Dockerfile for `udex-cli` (multi-stage, profile-switchable).
2. `projects/k8s/` directory: Helm chart, k8s manifests, management scripts.
3. SDK test audit — port JWT-only test cases to the OAuth2 fixture.
4. k8s test fixture in the SDK test suite.
5. CI job in `01-Validation.yml`.
6. Documentation: `projects/k8s/README.md`, `CONTRIBUTING.md`, `scripts/dev-doctor.sh`.

## Architecture

### Deployable unit

The `udex-cli` crate is the deployable unit. It embeds `udex-server` directly (as a Cargo dependency), so the `udex` binary can run `udex serve`. A single Docker image contains that binary; the Helm chart deploys it.

### External services

PostgreSQL and Hydra remain in the Compose environment for local development. Pods reach them via `host.k3d.internal` — k3d injects this hostname into every pod's `/etc/hosts` pointing at the Docker host's internal IP. The k8s deployment does not attempt to run either service on the cluster.

### Auth

The k8s deployment uses OAuth2 only. `AuthzConfig.jwks_url` is set to `http://host.k3d.internal:4444/.well-known/jwks.json` (Hydra's public JWKS endpoint). `danger_allow_non_tls` is `true` because the local Compose Hydra does not terminate TLS. Static-JWT auth (`jwt_public_key`) is not configured in the k8s deployment.

### Server configuration

The server reads a TOML config file via `UdexConfig::load`. In k8s:

- The config file is mounted from a ConfigMap at `/etc/udex/config.toml`.
- TLS cert and key are mounted from a k8s Secret at `/etc/udex/tls/tls.crt` and `/etc/udex/tls/tls.key`. The config references them as `urn:secrets-rs:file:/etc/udex/tls/tls.crt` etc.
- `DATABASE_URL` is injected as an environment variable from a k8s Secret. The config references it as `urn:secrets-rs:env:DATABASE_URL`.
- Postgres in Compose does not have TLS, so `datastore.dangerous_allow_non_tls = true`.
- `datastore.apply_migrations = true` for local dev (avoids a separate migration step).
- Server binds to `0.0.0.0:443`.

### Network topology

```text
Host machine
├── Docker (Compose)
│   ├── postgres :5432
│   └── hydra :4444 / :4445
└── k3d cluster (Docker containers as k8s nodes)
    └── udex pod
        ├── → postgres via host.k3d.internal:5432
        └── → hydra via host.k3d.internal:4444
```

The k3d cluster is created with `--port 8443:443@loadbalancer` so the udex gRPC service is reachable from the host at `localhost:8443`. The test harness and human operators connect there.

### Dockerfile

Located at `projects/rust/cli/Dockerfile`. Build context is `projects/` so both the Rust workspace (`rust/`) and shared protobuf definitions (`protobuf/`) are available to the build.

Two stages:

1. **builder** — `rust:1.95.0-bookworm` with `protoc` installed (required for the `api` crate). Accepts a `PROFILE` build arg (default `release`; pass `dev` for an unoptimised build). Builds with `cargo build --profile ${PROFILE} -p udex-cli`. Because the `dev` profile outputs to `target/debug/` (not `target/dev/`), the builder copies the binary to a fixed path (`/build/udex`) before the runtime stage references it.

2. **runtime** — `debian:bookworm-slim`. Copies only the binary. `EXPOSE 443`. `ENTRYPOINT ["udex", "serve", "--config", "/etc/udex/config.toml"]`.

### Helm chart layout

```text
projects/k8s/
├── helm/
│   └── udex/
│       ├── Chart.yaml
│       ├── values.yaml
│       └── templates/
│           ├── deployment.yaml
│           ├── service.yaml
│           ├── configmap.yaml   (server config TOML)
│           └── secret.yaml      (DATABASE_URL, TLS cert+key)
├── scripts/
│   ├── cluster-create.sh        (k3d cluster create with port mapping)
│   ├── cluster-delete.sh
│   ├── image-build.sh           (docker build with correct context + -f flag)
│   ├── image-load.sh            (k3d image import)
│   ├── deploy.sh                (helm upgrade --install)
│   └── undeploy.sh              (helm uninstall)
└── README.md
```

Scripts are thin wrappers — they validate prerequisites and delegate to the canonical tool. No business logic in shell.

### SDK test audit and k8s fixture

The existing SDK integration tests have two fixtures:

- `data()` — static-JWT fixture. Spins up an embedded server. Always runs.
- `data_hydra()` — OAuth2 fixture. Spins up an embedded server. Skips when `HYDRA_ADMIN_URL` is unset.

As part of this steel thread, every test case that runs only under `data()` is audited. If the test is exercising general API behaviour (not JWT-specific auth mechanics), it is ported to also run under `data_hydra()`. Tests that exist specifically to exercise static-JWT rejection or static-JWT-only behaviour remain on `data()`.

A third fixture is added:

- `data_k8s()` — OAuth2 fixture against a live k8s-deployed server. Reads `K8S_SERVER_URL` (e.g. `https://localhost:8443`). Skips when not set. Re-uses the same Hydra instance as `data_hydra()` (same `HYDRA_ADMIN_URL` / `HYDRA_PUBLIC_URL`). Uses the existing generated TLS CA cert for server certificate validation.

### CI job

A new `k8s-test` job is added to `01-Validation.yml`, running after the existing `test` job. It:

1. Starts Compose services (same as `test`).
2. Installs k3d, kubectl, Helm on the runner.
3. Builds the Docker image.
4. Creates a k3d cluster and loads the image.
5. Deploys via `scripts/deploy.sh`.
6. Waits for the Deployment to be ready (`kubectl rollout status`).
7. Runs `cargo test -p udex-sdk -- k8s` (test name filter) with `K8S_SERVER_URL` set.
8. Runs `undeploy.sh` and deletes the cluster in an `always` step.

The job is gated on paths: `projects/k8s/**`, `projects/rust/cli/Dockerfile`, `projects/rust/cli/src/**`, `projects/rust/sdk/tests/**`. This avoids running a full cluster spin-up for unrelated changes.

## Design Decisions

**OAuth2-only in k8s.** Static-JWT auth is not configured in the cluster. This matches real deployment practice: Hydra is the expected auth provider. Running both auth modes in k8s would double the secret surface area and test coverage of a configuration that will never be used in production.

**`dangerous_allow_non_tls = true` for Hydra JWKS.** The local Compose Hydra does not have TLS. This flag is explicitly scoped to the local k8s values; production values would use an HTTPS JWKS URL.

**`apply_migrations = true` in local k8s.** Avoids requiring an explicit migration step in the dev workflow. Production deployments should use an explicit pre-deploy `udex migrate apply` job.

**Build context is `projects/`, not `projects/rust/cli/`.** The Docker build needs both the Rust workspace and the top-level `protobuf/` directory. The Dockerfile path and context are independent; scripts pass `-f projects/rust/cli/Dockerfile projects/`.

**Port 8443 for local access.** 443 requires root on the host in many environments. k3d maps container 443 → host 8443 via `--port 8443:443@loadbalancer`. The TLS cert generated by `gen-keys-and-certs.sh` already includes `localhost` as a SAN, so client certificate validation passes at `localhost:8443`.

**k8s job path-gated in CI.** The cluster spin-up adds ~3 minutes to the pipeline. Gating on relevant paths avoids paying that cost for documentation-only or datastore-only changes.

## Alternatives Considered

**Run Hydra and PostgreSQL on k8s.** Rejected: significant added complexity (StatefulSets, PVCs, init containers for schema migration), none of which is the goal of this steel thread. The Compose services are already reliable; bridging to them via `host.k3d.internal` is sufficient for local development.

**Ingress + TLS termination at load balancer.** Rejected: the server speaks gRPC over TLS and the client validates the server certificate. Terminating TLS at an ingress would require re-encryption or plain-text gRPC between ingress and pod, adding complexity. Passthrough at the LoadBalancer (port 443 mapped directly) is simpler and correct.

**Separate Rust binary for the server only.** Rejected: the `cli` crate already combines the server and CLI. Splitting would require restructuring the Cargo workspace, which is out of scope.
