# Dev Container

VS Code dev container configuration for Udex. Opening the repository in VS Code (or GitHub Codespaces) and choosing **Reopen in Container** gives a fully configured development environment with no manual setup required.

## First Time Setup
Run `scripts/gen-env.sh` before starting the devcontainer

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
2. Generates `.env` at the workspace root (via `scripts/gen-env.sh`) if one does not already exist, using Docker service names for the Hydra and OpenObserve URLs.
3. Creates `.devcontainer/.env` as a symlink to `../.env` so the Compose env-file reference resolves correctly.
4. Generates TLS certificates and JWT signing keys (via `scripts/gen-keys-and-certs.sh`) if they do not already exist.
5. Installs the Intent Claude subagent and `in-essentials` skill.

The OpenObserve-backed observability fixture (OpenObserve + collector + Vector) is part of the base `projects/compose` stack, so it comes up automatically with the devcontainer — no separate step. Application/SDK telemetry export stays opt-in. Browse the telemetry at <http://localhost:5080>; see [projects/compose/README.md#observability](../projects/compose/README.md#observability) for the login and what lands where.

Re-creating the container re-runs the script. Because the setup scripts are idempotent, existing secrets and key material are left untouched unless `--force` is passed manually.

## Forwarded ports

| Port | Service |
|---|---|
| `5432` | PostgreSQL |
| `4444` | Hydra public (token issuance) |
| `4445` | Hydra admin (client management) |

The observability fixture publishes its own ports directly to the host (no `forwardPorts` entry needed): OpenObserve UI and search API `5080`, OTLP `4317`/`4318`.

Note that from *inside* the devcontainer those published ports are on the host, not on this container's `localhost` — reach the fixture by compose service name instead (`http://openobserve:5080`). `gen-env.sh` writes the right form into `.env` as `OPENOBSERVE_URL`, the same way it does for `HYDRA_PUBLIC_URL`.

## Environment variables

| Variable | Value inside container | Notes |
|---|---|---|
| `DATABASE_URL` | `postgres://postgres:<pw>@localhost:5432/postgres` | `localhost` is postgres (shared network) |
| `HYDRA_PUBLIC_URL` | `http://hydra:4444` | Overrides `.env` default of `http://localhost:4444` |
| `HYDRA_ADMIN_URL` | `http://hydra:4445` | Overrides `.env` default of `http://localhost:4445` |
| `OPENOBSERVE_URL` | `http://openobserve:5080` | Overrides `.env` default of `http://localhost:5080` |
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
