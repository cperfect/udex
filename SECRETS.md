# Secrets & Key Inventory

All credentials, keys, certificates, and closely associated principals used in this project.
Public artefacts (certificates, public keys, client IDs, endpoint URLs) are marked in the **Public** column.

> **Rule:** Never commit real credentials. Rows marked **Prod** or **Both** must be supplied at runtime via environment variables or a secrets manager — never hardcoded.

## Database

| Name | Type | Usage | Scope | Public | Source |
|------|------|-------|-------|--------|--------|
| `DB_USER` | DB username | Rust server/CLI (connection URL placeholder) | Both | No | `projects/rust/cli/src/config.rs` — `${DB_USER}` placeholder |
| `DB_PASSWORD` | DB password | Rust server/CLI (connection URL placeholder) | Both | No | `projects/rust/cli/src/config.rs` — `${DB_PASSWORD}` placeholder |
| `DB_HOST` | DB hostname | Rust server/CLI (connection URL placeholder) | Both | Yes | `projects/rust/cli/src/config.rs` — `${DB_HOST}` placeholder |
| `DB_PORT` | DB port | Rust server/CLI (connection URL placeholder) | Both | Yes | `projects/rust/cli/src/config.rs` — `${DB_PORT}` placeholder |
| `DB_NAME` | DB name | Rust server/CLI (connection URL placeholder) | Both | Yes | `projects/rust/cli/src/config.rs` — `${DB_NAME}` placeholder |
| `DATABASE_URL` | Full PostgreSQL connection string | Devcontainer, CI, integration tests | Dev | No | `.devcontainer/devcontainer.json` (hardcoded `postgres://postgres:admin@localhost:5432/postgres`); `.github/workflows/01-Validation.yml` (hardcoded `postgres://postgres:postgres@localhost:5432/postgres`) |
| `POSTGRES_PASSWORD` | DB password for `postgres` superuser | Docker Compose, CI | Dev | No | `projects/compose/docker-compose.yml` (hardcoded `admin`); `.github/workflows/01-Validation.yml` (hardcoded `postgres`) |
| Hydra DB password | DB password for `hydra` DB user | Docker Compose (Postgres init script) | Dev | No | `projects/compose/postgres/docker-entrypoint-initdb.d/01-init-hydra-db.sh` (hardcoded `secret`); `projects/compose/docker-compose.yml` — Hydra `DSN` env var |

## OAuth2 / OIDC (Hydra)

| Name | Type | Usage | Scope | Public | Source |
|------|------|-------|-------|--------|--------|
| `SECRETS_SYSTEM` | Hydra system signing/encryption key (hex) | Docker Compose — Hydra container | Dev | No | `projects/compose/docker-compose.yml` (hardcoded) |
| `UDEX_CLIENT_ID` | OAuth2 client ID | CLI `token fetch` command | Both | Yes | `projects/rust/cli/src/cli.rs` — env var / flag |
| `UDEX_CLIENT_SECRET` | OAuth2 client secret | CLI `token fetch` command | Both | No | `projects/rust/cli/src/cli.rs` — env var / flag |
| `UDEX_TOKEN` | Bearer access token | CLI — passed to gRPC requests | Both | No | `projects/rust/cli/src/cli.rs` — env var / flag |
| Hydra test client secret (`hydra-test-secret`) | OAuth2 client secret | Server integration tests — `client_credentials` grant | Dev | No | `projects/rust/server/tests/server_integration_tests.rs` (hardcoded) |
| Hydra non-Udex client secret (`non-udex-secret`) | OAuth2 client secret | Server integration tests — scope rejection test | Dev | No | `projects/rust/server/tests/server_integration_tests.rs` (hardcoded) |
| Hydra wrong-audience client secret (`wrong-aud-secret`) | OAuth2 client secret | Server integration tests — audience rejection test | Dev | No | `projects/rust/server/tests/server_integration_tests.rs` (hardcoded) |
| `jwks_url` | JWKS endpoint URL | Rust server config — runtime key source | Both | Yes | `projects/rust/server/src/config.rs`; `projects/rust/cli/src/config.rs` — config property |

## JWT Signing Keys

| Name | Type | Usage | Scope | Public | Source |
|------|------|-------|-------|--------|--------|
| `signing_private_key.pem` | ECDSA P-256 JWT private key (PKCS8 PEM) | Server unit & integration tests — signing test tokens | Dev | No | `projects/rust/server/tests/jwt/signing_private_key.pem` |
| `signing_public_key.pem` | ECDSA P-256 JWT public key (PEM) | Server unit & integration tests — verifying JWT signatures | Dev | Yes | `projects/rust/server/tests/jwt/signing_public_key.pem` |
| `bad_signing_private_key.pem` | ECDSA P-256 JWT private key — wrong key pair | Server tests — invalid signature rejection | Dev | No | `projects/rust/server/tests/jwt/bad_signing_private_key.pem` |
| `bad_signing_public_key.pem` | ECDSA P-256 JWT public key — wrong key pair | Server tests — invalid signature rejection | Dev | Yes | `projects/rust/server/tests/jwt/bad_signing_public_key.pem` |
| `jwt_public_key_path` | Path to JWT public key (config property) | Rust server/CLI — static key source | Both | Yes | `projects/rust/server/src/config.rs`; `projects/rust/cli/src/config.rs` — config property |

## TLS Certificates & Keys

| Name | Type | Usage | Scope | Public | Source |
|------|------|-------|-------|--------|--------|
| `ca.key` | RSA-4096 CA private key | Test certificate generation only | Dev | No | `projects/rust/server/tests/certs/ca.key` |
| `ca.crt` | Self-signed CA certificate (365-day) | Server integration tests, bench — TLS trust anchor | Dev | Yes | `projects/rust/server/tests/certs/ca.crt` |
| `server.key` | RSA-4096 TLS server private key | gRPC server TLS | Dev | No | `projects/rust/server/tests/certs/server.key`; default path `certs/server.key` in `projects/rust/server/src/config.rs` and `projects/rust/cli/src/config.rs` |
| `server.crt` | TLS server certificate (signed by test CA) | gRPC server TLS | Dev | Yes | `projects/rust/server/tests/certs/server.crt`; default path `certs/server.crt` in `projects/rust/server/src/config.rs` and `projects/rust/cli/src/config.rs` |
| `server.csr` | TLS server certificate signing request | Intermediate artefact — cert generation only | Dev | Yes | `projects/rust/server/tests/certs/server.csr` |
| `tls.cert_path` | Path to TLS certificate (config property) | Rust server/CLI — TLS configuration | Both | Yes | `projects/rust/server/src/config.rs`; `projects/rust/cli/src/config.rs` — config property |
| `tls.key_path` | Path to TLS private key (config property) | Rust server/CLI — TLS configuration | Both | No | `projects/rust/server/src/config.rs`; `projects/rust/cli/src/config.rs` — config property |

## Key Generation Scripts

| Name | Type | Usage | Scope | Public | Source |
|------|------|-------|-------|--------|--------|
| `regenerate_jwt_signing_key_pair.sh` | Key generation script (ECDSA P-256, PKCS8) | Dev — rotate test JWT keys | Dev | Yes | `projects/rust/server/tests/jwt/regenerate_jwt_signing_key_pair.sh` |
| `regenerate_certs.sh` | Certificate generation script (RSA-4096, CA + server) | Dev — rotate test TLS certs | Dev | Yes | `projects/rust/server/tests/certs/regenerate_certs.sh` |
