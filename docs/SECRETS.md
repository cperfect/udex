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
| SDK Hydra scoped-client secret (`sdk-hydra-scoped-secret`) | OAuth2 client secret | SDK integration tests — wildcard-scoped client used to populate a test-owned index, so tests need not borrow the shared fixture index | Dev | No | `projects/rust/sdk/tests/integration_tests.rs` (dev-only hardcoded fixture) |
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

The observability fixture (ST0027, backend replaced by ST0028; part of the base `projects/compose` stack) holds one generated credential. The OTel collector's OTLP ingest is still **keyless and plaintext** — the application emits anonymous OTLP and never carries a credential — but OpenObserve requires authentication on both ingest and query, so the collector holds one on the application's behalf.

| Name | Type | Usage | Scope | Public | Source |
|------|------|-------|-------|--------|--------|
| `OPENOBSERVE_ROOT_EMAIL` | Account identifier | OpenObserve root user; UI login and test queries | Dev | Yes | `scripts/gen-env.sh` → `.env` (gitignored) |
| `OPENOBSERVE_ROOT_PASSWORD_SECRET` | Generated password | OpenObserve root user; UI login and test queries | Dev | No | `scripts/gen-env.sh` → `.env` (gitignored) |
| `OPENOBSERVE_BASIC_AUTH_SECRET` | `base64(email:password)` | OTel Collector's `Authorization: Basic` header for the OpenObserve exporter | Dev | No | Derived by `scripts/gen-env.sh` from the two above → `.env` (gitignored) |

Two properties are worth stating rather than leaving implicit.

**This is a weaker posture than the ClickHouse fixture it replaces.** That one was keyless, so nothing sensitive crossed the compose network at all. Now a credential does, over a plaintext in-network hop. It is accepted because the fixture is dev/CI-only, the port is published on loopback only, and the value is generated per environment and never leaves it — but it is a real change, not a like-for-like swap.

**The password cannot be plain hex like the other generated secrets.** OpenObserve enforces 8–128 characters with at least one lowercase, uppercase, digit and special character, and *panics on first boot* when that is not met. `gen-env.sh` satisfies it while keeping the value inside `[A-Za-z0-9-]`, because the generated values are expanded through an unquoted heredoc and consumed by docker compose's `.env` parser — a `$` or `#` in that value breaks the fixture in ways that are tedious to diagnose.

Rotate by re-running `gen-env.sh`, never by hand-editing: `OPENOBSERVE_BASIC_AUTH_SECRET` is derived from the other two and will otherwise authenticate nothing. Rotation also requires recreating the containers — see [Rotating secrets](#rotating-secrets) below.

In CI there is no `.env`; the workflow generates the same three values per job rather than storing them as repository secrets, since the fixture is destroyed with the runner.

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
| `gen-env.sh` | Env var generation script | Dev — generate `.env` with DB passwords, Hydra secrets and OpenObserve credentials | Dev | Yes | `scripts/gen-env.sh` |
| `gen-keys-and-certs.sh` | Key/cert generation script (delegates to sub-scripts) | Dev/CI — generate server TLS certs, Traefik edge certs, and JWT signing keys | Dev | Yes | `scripts/gen-keys-and-certs.sh` |
| `regenerate_jwt_signing_key_pair.sh` | Key generation script (ECDSA P-256, PKCS8) | Dev — rotate test JWT keys (invoked by `gen-keys-and-certs.sh`) | Dev | Yes | `projects/rust/server/tests/jwt/regenerate_jwt_signing_key_pair.sh` |
| `regenerate_certs.sh` (server) | Certificate generation script (RSA-4096, CA + server) | Dev — rotate pod TLS certs (invoked by `gen-keys-and-certs.sh`) | Dev | Yes | `projects/rust/server/tests/certs/regenerate_certs.sh` |
| `regenerate_certs.sh` (Traefik edge) | Certificate generation script (RSA-4096, CA + edge cert) | Dev — rotate Traefik edge TLS certs for k8s (invoked by `gen-keys-and-certs.sh`) | Dev | Yes | `projects/k8s/traefik/certs/regenerate_certs.sh` |
| `hydra-create-client.sh` | Hydra OAuth2 client registration script | Dev — register a client in Hydra with specified scopes; prints env vars for CLI use | Dev | Yes | `scripts/hydra-create-client.sh` |

## Rotating secrets

`gen-env.sh --force` writes fresh random values into `.env`, but the running stateful services do **not** pick them up automatically:

- **Postgres** applies `POSTGRES_PASSWORD` (and runs the init script that sets the `hydra` role password) **only on first initialization of an empty data volume.** A volume that already exists keeps its original passwords. `pg_hba.conf` trusts loopback but requires `scram-sha-256` for every other client, so host-side tools (`cargo test`, `psql`, the CLI) keep working against the stale password while scram clients — notably **k8s pods** — fail to authenticate, far from the cause.
- **Hydra** reads `HYDRA_SECRETS_SYSTEM_SECRET` and its DSN password at container start; a rotated `.env` only takes effect after the container is recreated.
- **OpenObserve and the OTel collector** are the same shape. OpenObserve applies `ZO_ROOT_USER_*` on first boot into its container-local SQLite, and the collector reads `OPENOBSERVE_BASIC_AUTH_SECRET` at start. Rotating `.env` without recreating **both** leaves the collector presenting a credential the store no longer accepts — telemetry then stops arriving with nothing obviously broken. The recipe below already covers it, since `down -v` removes the OpenObserve container along with everything else.

Because of this, `gen-env.sh` **refuses to rotate an existing `.env` while a compose Postgres is running.** To rotate cleanly, wipe the stateful volumes so the stack re-initializes from the new secrets:

```bash
docker compose -f projects/compose/docker-compose.yml --env-file .env down -v
bash scripts/gen-env.sh --force
docker compose -f projects/compose/docker-compose.yml --env-file .env up -d
# then redeploy k8s so its secret matches: bash projects/k8s/scripts/deploy.sh
```

`--rotate-live` overrides the guard, but leaves the drift in place — use it only when you will realign the database yourself. `scripts/dev-doctor.sh` detects this exact drift: it authenticates over scram and reports a failure when `.env` is out of sync with the database volume.
