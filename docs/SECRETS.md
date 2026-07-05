# Secrets & Key Inventory

All credentials, keys, certificates, and closely associated principals used in this project.
Public artefacts (certificates, public keys, client IDs, endpoint URLs) are marked in the **Public** column.

> **Rule:** Never commit real credentials. Rows marked **Prod** or **Both** must be supplied at runtime via environment variables or a secrets manager — never hardcoded.

## Database

| Name | Type | Usage | Scope | Public | Source |
|------|------|-------|-------|--------|--------|
| `POSTGRES_PASSWORD_SECRET` | DB password for `postgres` superuser | Docker Compose, devcontainer, CI | Dev | No | `scripts/gen-env.sh` → `.env` (gitignored); CI via `secrets.POSTGRES_PASSWORD_SECRET` |
| `HYDRA_DB_PASSWORD_SECRET` | DB password for `hydra` DB user | Docker Compose — Postgres init script + Hydra DSN | Dev | No | `scripts/gen-env.sh` → `.env` (gitignored) |
| `DATABASE_URL` | Full PostgreSQL connection string | Rust server/CLI (`urn:secrets-rs:env:DATABASE_URL`), integration tests | Dev | No | `scripts/gen-env.sh` → `.env` (gitignored); CI constructed from `secrets.POSTGRES_PASSWORD_SECRET` |

## OAuth2 / OIDC (Hydra)

| Name | Type | Usage | Scope | Public | Source |
|------|------|-------|-------|--------|--------|
| `HYDRA_SECRETS_SYSTEM_SECRET` | Hydra system signing/encryption key (hex) | Docker Compose — Hydra container | Dev | No | `scripts/gen-env.sh` → `.env` (gitignored) |
| `UDEX_CLIENT_ID` | OAuth2 client ID | CLI `token fetch` — `--client-id` flag or env var | Both | Yes | Operator-supplied env var |
| `UDEX_CLIENT_SECRET` | OAuth2 client secret | CLI `token fetch` — env var only | Both | No | Operator-supplied env var (`export UDEX_CLIENT_SECRET=...`) |
| `UDEX_TOKEN` | Bearer access token | CLI — injected into gRPC `Authorization` header | Both | No | Operator-supplied env var (`export UDEX_TOKEN=...`) |
| Hydra test client secret (`hydra-test-secret`) | OAuth2 client secret | Server integration tests — `client_credentials` grant | Dev | No | `projects/rust/server/tests/server_integration_tests.rs` (dev-only hardcoded fixture) |
| Hydra non-Udex client secret (`non-udex-secret`) | OAuth2 client secret | Server integration tests — scope rejection test | Dev | No | `projects/rust/server/tests/server_integration_tests.rs` (dev-only hardcoded fixture) |
| Hydra wrong-audience client secret (`wrong-aud-secret`) | OAuth2 client secret | Server integration tests — audience rejection test | Dev | No | `projects/rust/server/tests/server_integration_tests.rs` (dev-only hardcoded fixture) |
| SDK Hydra test client secret (`sdk-hydra-test-secret`) | OAuth2 client secret | SDK integration tests — Hydra fixture | Dev | No | `projects/rust/sdk/tests/integration_tests.rs` (dev-only hardcoded fixture) |
| CLI token test client secret (`cli-hydra-test-secret`) | OAuth2 client secret | CLI token Hydra tests | Dev | No | `projects/rust/cli/tests/token_hydra_tests.rs` (dev-only hardcoded fixture) |
| CLI index test client secret (`cli-hydra-idx-del-secret`) | OAuth2 client secret | CLI index delete Hydra tests | Dev | No | `projects/rust/cli/tests/index_hydra_tests.rs` (dev-only hardcoded fixture) |
| `jwks_url` | JWKS endpoint URL | Rust server config — runtime key source | Both | Yes | Config property (`projects/rust/server/src/config.rs`; `projects/rust/cli/src/config.rs`) |

## JWT Signing Keys

| Name | Type | Usage | Scope | Public | Source |
|------|------|-------|-------|--------|--------|
| `signing_private_key.pem` | ECDSA P-256 JWT private key (PKCS8 PEM) | Server unit & integration tests — signing test tokens | Dev | No | `scripts/gen-keys-and-certs.sh` → `projects/rust/server/tests/jwt/` (gitignored) |
| `signing_public_key.pem` | ECDSA P-256 JWT public key (PEM) | Server unit & integration tests — verifying JWT signatures | Dev | Yes | `scripts/gen-keys-and-certs.sh` → `projects/rust/server/tests/jwt/` (gitignored) |
| `bad_signing_private_key.pem` | ECDSA P-256 JWT private key — wrong key pair | Server tests — invalid signature rejection | Dev | No | `scripts/gen-keys-and-certs.sh` → `projects/rust/server/tests/jwt/` (gitignored) |
| `bad_signing_public_key.pem` | ECDSA P-256 JWT public key — wrong key pair | Server tests — invalid signature rejection | Dev | Yes | `scripts/gen-keys-and-certs.sh` → `projects/rust/server/tests/jwt/` (gitignored) |
| `authz.jwt_public_key` | `Secret<String>` holding a `urn:secrets-rs:file:` URN; resolved to PEM at startup | Rust server/CLI — static-key authentication mode | Both | Yes | Config property; see `projects/rust/server/src/config.rs` |

## Observability

The ClickHouse-backed observability fixture (ST0027; part of the base
`projects/compose` stack) holds **no secrets**: the OTel collector is keyless and
plaintext locally, and the HyperDX dev UI registers its own local user
(`admin@udex.local`) rather than reading an env credential. The HyperDX login is a
fixed dev-only convenience credential, not a generated secret.

## TLS Certificates & Keys

| Name | Type | Usage | Scope | Public | Source |
|------|------|-------|-------|--------|--------|
| `ca.key` | RSA-4096 CA private key | Test certificate generation only | Dev | No | `scripts/gen-keys-and-certs.sh` → `projects/rust/server/tests/certs/` (gitignored) |
| `ca.crt` | Self-signed CA certificate (365-day) | Server integration tests, bench — TLS trust anchor | Dev | Yes | `scripts/gen-keys-and-certs.sh` → `projects/rust/server/tests/certs/` (gitignored) |
| `server.key` | RSA-4096 TLS server private key | gRPC server TLS (pod cert; re-encrypted backend hop in k8s) | Dev | No | `scripts/gen-keys-and-certs.sh` → `projects/rust/server/tests/certs/` (gitignored) |
| `server.crt` | TLS server certificate (signed by test CA) | gRPC server TLS (pod cert; re-encrypted backend hop in k8s) | Dev | Yes | `scripts/gen-keys-and-certs.sh` → `projects/rust/server/tests/certs/` (gitignored) |
| `server.csr` | TLS server certificate signing request | Intermediate artefact — cert generation only | Dev | Yes | `scripts/gen-keys-and-certs.sh` → `projects/rust/server/tests/certs/` (gitignored) |
| Traefik edge `ca.key` | RSA-4096 CA private key | Edge certificate generation only (k8s) | Dev | No | `scripts/gen-keys-and-certs.sh` → `projects/k8s/traefik/certs/` (gitignored) |
| Traefik edge `ca.crt` | Self-signed edge CA certificate (365-day) | k8s — trust anchor clients use when Traefik terminates TLS at the ingress | Dev | Yes | `scripts/gen-keys-and-certs.sh` → `projects/k8s/traefik/certs/` (gitignored) |
| Traefik edge `tls.key` | RSA-4096 TLS private key | k8s — Traefik edge TLS (client-facing termination) | Dev | No | `scripts/gen-keys-and-certs.sh` → `projects/k8s/traefik/certs/` (gitignored) |
| Traefik edge `tls.crt` | TLS edge certificate (signed by edge CA) | k8s — cert Traefik presents to clients at the ingress | Dev | Yes | `scripts/gen-keys-and-certs.sh` → `projects/k8s/traefik/certs/` (gitignored) |
| Traefik edge `tls.csr` | TLS certificate signing request | Intermediate artefact — edge cert generation only | Dev | Yes | `scripts/gen-keys-and-certs.sh` → `projects/k8s/traefik/certs/` (gitignored) |
| `tls.cert` | `Secret<String>` holding a `urn:secrets-rs:file:` URN; resolved to PEM certificate at startup | Rust server/CLI — TLS configuration | Both | Yes | Config property; see `projects/rust/server/src/config.rs` |
| `tls.key` | `Secret<String>` holding a `urn:secrets-rs:file:` URN; resolved to PEM private key at startup | Rust server/CLI — TLS configuration | Both | No | Config property; see `projects/rust/server/src/config.rs` |

## Key Generation Scripts

| Name | Type | Usage | Scope | Public | Source |
|------|------|-------|-------|--------|--------|
| `gen-env.sh` | Env var generation script | Dev — generate `.env` with DB passwords and Hydra secrets | Dev | Yes | `scripts/gen-env.sh` |
| `gen-keys-and-certs.sh` | Key/cert generation script (delegates to sub-scripts) | Dev/CI — generate server TLS certs, Traefik edge certs, and JWT signing keys | Dev | Yes | `scripts/gen-keys-and-certs.sh` |
| `regenerate_jwt_signing_key_pair.sh` | Key generation script (ECDSA P-256, PKCS8) | Dev — rotate test JWT keys (invoked by `gen-keys-and-certs.sh`) | Dev | Yes | `projects/rust/server/tests/jwt/regenerate_jwt_signing_key_pair.sh` |
| `regenerate_certs.sh` (server) | Certificate generation script (RSA-4096, CA + server) | Dev — rotate pod TLS certs (invoked by `gen-keys-and-certs.sh`) | Dev | Yes | `projects/rust/server/tests/certs/regenerate_certs.sh` |
| `regenerate_certs.sh` (Traefik edge) | Certificate generation script (RSA-4096, CA + edge cert) | Dev — rotate Traefik edge TLS certs for k8s (invoked by `gen-keys-and-certs.sh`) | Dev | Yes | `projects/k8s/traefik/certs/regenerate_certs.sh` |
| `hydra-create-client.sh` | Hydra OAuth2 client registration script | Dev — register a client in Hydra with specified scopes; prints env vars for CLI use | Dev | Yes | `scripts/hydra-create-client.sh` |

## Rotating secrets

`gen-env.sh --force` writes fresh random values into `.env`, but the running stateful services do **not** pick them up automatically:

- **Postgres** applies `POSTGRES_PASSWORD` (and runs the init script that sets the `hydra` role password) **only on first initialization of an empty data volume.** A volume that already exists keeps its original passwords. `pg_hba.conf` trusts loopback but requires `scram-sha-256` for every other client, so host-side tools (`cargo test`, `psql`, the CLI) keep working against the stale password while scram clients — notably **k8s pods** — fail to authenticate, far from the cause.
- **Hydra** reads `HYDRA_SECRETS_SYSTEM_SECRET` and its DSN password at container start; a rotated `.env` only takes effect after the container is recreated.

Because of this, `gen-env.sh` **refuses to rotate an existing `.env` while a compose Postgres is running.** To rotate cleanly, wipe the stateful volumes so the stack re-initializes from the new secrets:

```bash
docker compose -f projects/compose/docker-compose.yml --env-file .env down -v
bash scripts/gen-env.sh --force
docker compose -f projects/compose/docker-compose.yml --env-file .env up -d
# then redeploy k8s so its secret matches: bash projects/k8s/scripts/deploy.sh
```

`--rotate-live` overrides the guard, but leaves the drift in place — use it only when you will realign the database yourself. `scripts/dev-doctor.sh` detects this exact drift: it authenticates over scram and reports a failure when `.env` is out of sync with the database volume.
