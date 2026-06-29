# Dev Container

VS Code dev container configuration for Udex. Opening the repository in VS Code (or GitHub Codespaces) and choosing **Reopen in Container** gives a fully configured development environment with no manual setup required.

## What it runs

The devcontainer uses two Docker Compose files layered together:

| File | Purpose |
|---|---|
| `docker-compose.devcontainer.yml` | The development container itself (Rust toolchain, tools) |
| `../projects/compose/docker-compose.yml` | PostgreSQL 16 + Ory Hydra v26.2.0 |

All services share a single Docker network rooted at the `postgres` container — the devcontainer uses `network_mode: service:postgres`, which means `localhost` inside the container resolves to the PostgreSQL container. Hydra is reached by its service name (`hydra:4444`, `hydra:4445`), not `localhost`; the environment variables `HYDRA_PUBLIC_URL` and `HYDRA_ADMIN_URL` are set accordingly.

## What is installed

The `Dockerfile` extends the Microsoft Rust devcontainer image (`bookworm`) and adds:

| Tool | Version | How |
|---|---|---|
| Rust | 1.95.0 (pinned) | `rustup` (upgraded in post-create) |
| `clippy`, `rustfmt` | matching Rust version | `rustup component add` |
| `protoc` | latest | devcontainer feature |
| Trivy | latest | official Trivy apt repo |
| Claude Code | latest | devcontainer feature |
| Intent | latest `main` | cloned from GitHub to `/opt/intent` |
| Hydra CLI | v26.2.0 (checksum-verified) | downloaded in post-create |
| Node.js | latest LTS | devcontainer feature |
| SQLite | latest | devcontainer feature |
| vim | latest | devcontainer feature |

## Post-create script

`post-create.sh` runs automatically after the container is created:

1. Pins Rust to the version in `$RUST_VERSION` and adds `clippy` / `rustfmt`.
2. Generates `.env` at the workspace root (via `scripts/gen-env.sh`) if one does not already exist, using Docker service names for Hydra URLs.
3. Creates `.devcontainer/.env` as a symlink to `../.env` so the Compose env-file reference resolves correctly.
4. Generates TLS certificates and JWT signing keys (via `scripts/gen-keys-and-certs.sh`) if they do not already exist.
5. Installs the Intent Claude subagent and `in-essentials` skill.
6. Starts the local observability stack (via `projects/observability/scripts/up.sh`) so Grafana, Prometheus, Tempo, Loki, and the OpenTelemetry Collector are available by default. This step is non-fatal — a transient failure prints a warning rather than aborting setup. Application/SDK telemetry export stays opt-in.

Re-creating the container re-runs the script. Because the setup scripts are idempotent, existing secrets and key material are left untouched unless `--force` is passed manually.

## Forwarded ports

| Port | Service |
|---|---|
| `5432` | PostgreSQL |
| `4444` | Hydra public (token issuance) |
| `4445` | Hydra admin (client management) |

The observability stack publishes its own ports directly to the host (no `forwardPorts` entry needed): Grafana `3000`, Prometheus `9090`, Loki `3100`, Tempo `3200`, OTLP `4317`/`4318`.

## Environment variables

| Variable | Value inside container | Notes |
|---|---|---|
| `DATABASE_URL` | `postgres://postgres:<pw>@localhost:5432/postgres` | `localhost` is postgres (shared network) |
| `HYDRA_PUBLIC_URL` | `http://hydra:4444` | Overrides `.env` default of `http://localhost:4444` |
| `HYDRA_ADMIN_URL` | `http://hydra:4445` | Overrides `.env` default of `http://localhost:4445` |
| `RUST_LOG` | `info,udex=debug` | Debug logging for Udex crates, info for everything else |
| `RUST_BACKTRACE` | `1` | Full backtraces on panic |

## Files

```text
.devcontainer/
├── devcontainer.json               # VS Code dev container configuration
├── docker-compose.devcontainer.yml # Devcontainer service definition
├── Dockerfile                      # Container image (Rust + tooling)
└── post-create.sh                  # One-time setup run after container creation
```

## Rebuilding

To pick up changes to the `Dockerfile` or `devcontainer.json`, use **Dev Containers: Rebuild Container** from the VS Code command palette. This re-runs `post-create.sh` but the idempotency guards mean secrets and keys are only regenerated if missing.
