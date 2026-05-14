# Udex Local Dev Environment

Docker Compose configuration for local development and integration testing. Runs **PostgreSQL 16** (the Udex datastore) and **Ory Hydra v26.2.0** (the OAuth2 authorization server used for authentication).

## Services

| Service | Image | Ports | Purpose |
|---|---|---|---|
| `postgres` | `postgres:16` | `5432` | Udex datastore + Hydra metadata DB |
| `hydra-migrate` | `oryd/hydra:v26.2.0` | — | One-shot migration; runs `migrate sql up` then exits |
| `hydra` | `oryd/hydra:v26.2.0` | `4444`, `4445`, `5555` | OAuth2 server (public, admin, token endpoints) |

`hydra-migrate` must complete successfully before `hydra` starts. PostgreSQL must be healthy before either Hydra service starts.

## Prerequisites

Generate the required secrets and environment file before starting:

```bash
# From the workspace root
bash scripts/gen-env.sh
```

This creates `.env` at the workspace root with generated passwords and Hydra system secrets. It is idempotent — re-run with `--force` to rotate. The devcontainer runs this automatically on first create.

## Starting and stopping

```bash
# Start all services (from the workspace root)
docker compose -f projects/compose/docker-compose.yml --env-file .env up -d

# Stop and remove containers (data is lost — see Persistence below)
docker compose -f projects/compose/docker-compose.yml down
```

When using the devcontainer, services start automatically and the above commands are not needed.

## Service URLs

| Endpoint | URL |
|---|---|
| PostgreSQL | `postgres://postgres:<password>@localhost:5432/postgres` |
| Hydra public (token issuance) | `http://localhost:4444` |
| Hydra admin (client management) | `http://localhost:4445` |

The full `DATABASE_URL` is written into `.env` by `gen-env.sh` and picked up automatically by `cargo test` and the CLI.

## Hydra setup

Hydra runs in `--dev` mode, which disables HTTPS and consent UI requirements — appropriate for the OAuth2 Client Credentials flow used by Udex. OAuth2 clients (service identities and test fixtures) are registered against the admin API at `http://localhost:4445`.

The helper script `scripts/hydra-create-client.sh` registers a client with the scopes needed for integration tests.

## Persistence

Data is **ephemeral** by default — `docker compose down` wipes both the Udex and Hydra databases. The persistent volume configuration is present but commented out in `docker-compose.yml`. For local testing this is intentional: a clean `up` always starts from a known-good schema.

## Directory layout

```text
compose/
├── docker-compose.yml
└── postgres/
    └── docker-entrypoint-initdb.d/
        └── 01-init-hydra-db.sh   # Creates the hydra user and database on first start
```
