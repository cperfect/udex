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

| Name | Type | Usage | Scope | Public | Source |
|------|------|-------|-------|--------|--------|
| `GRAFANA_ADMIN_USER` | Grafana admin username | Local observability stack — Grafana container (`GF_SECURITY_ADMIN_USER`) | Dev | Yes | `scripts/gen-env.sh` → `.env` (gitignored); defaults to `admin` |
| `GRAFANA_ADMIN_PASSWORD` | Grafana admin password (hex) | Local observability stack — Grafana container (`GF_SECURITY_ADMIN_PASSWORD`) | Dev | No | `scripts/gen-env.sh` → `.env` (gitignored) |

OTLP collector TLS certificates (`projects/observability/certs/`) are listed under [TLS Certificates & Keys](#tls-certificates--keys).

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
| OTLP `ca.key` | RSA-4096 CA private key | OTLP collector certificate generation only | Dev | No | `scripts/gen-keys-and-certs.sh` → `projects/observability/certs/` (gitignored) |
| OTLP `ca.crt` | Self-signed OTLP CA certificate (365-day) | Trust anchor the app uses to verify the observability Collector's OTLP TLS cert (`observability.otlp_ca`); mounted into k8s pods | Dev | Yes | `scripts/gen-keys-and-certs.sh` → `projects/observability/certs/` (gitignored) |
| OTLP `collector.key` | RSA-4096 TLS private key | OTLP Collector TLS (server side of the OTLP endpoint) | Dev | No | `scripts/gen-keys-and-certs.sh` → `projects/observability/certs/` (gitignored) |
| OTLP `collector.crt` | TLS certificate (signed by OTLP CA; SANs: otel-collector, localhost, host.docker.internal, host.k3d.internal) | OTLP Collector TLS — cert presented on the OTLP gRPC/HTTP endpoints | Dev | Yes | `scripts/gen-keys-and-certs.sh` → `projects/observability/certs/` (gitignored) |
| OTLP `collector.csr` | TLS certificate signing request | Intermediate artefact — OTLP cert generation only | Dev | Yes | `scripts/gen-keys-and-certs.sh` → `projects/observability/certs/` (gitignored) |
| `tls.cert` | `Secret<String>` holding a `urn:secrets-rs:file:` URN; resolved to PEM certificate at startup | Rust server/CLI — TLS configuration | Both | Yes | Config property; see `projects/rust/server/src/config.rs` |
| `tls.key` | `Secret<String>` holding a `urn:secrets-rs:file:` URN; resolved to PEM private key at startup | Rust server/CLI — TLS configuration | Both | No | Config property; see `projects/rust/server/src/config.rs` |

## Key Generation Scripts

| Name | Type | Usage | Scope | Public | Source |
|------|------|-------|-------|--------|--------|
| `gen-env.sh` | Env var generation script | Dev — generate `.env` with DB passwords, Hydra secrets, and the Grafana admin credential | Dev | Yes | `scripts/gen-env.sh` |
| `gen-keys-and-certs.sh` | Key/cert generation script (delegates to sub-scripts) | Dev/CI — generate server TLS certs, Traefik edge certs, OTLP collector certs, and JWT signing keys | Dev | Yes | `scripts/gen-keys-and-certs.sh` |
| `regenerate_jwt_signing_key_pair.sh` | Key generation script (ECDSA P-256, PKCS8) | Dev — rotate test JWT keys (invoked by `gen-keys-and-certs.sh`) | Dev | Yes | `projects/rust/server/tests/jwt/regenerate_jwt_signing_key_pair.sh` |
| `regenerate_certs.sh` (server) | Certificate generation script (RSA-4096, CA + server) | Dev — rotate pod TLS certs (invoked by `gen-keys-and-certs.sh`) | Dev | Yes | `projects/rust/server/tests/certs/regenerate_certs.sh` |
| `regenerate_certs.sh` (Traefik edge) | Certificate generation script (RSA-4096, CA + edge cert) | Dev — rotate Traefik edge TLS certs for k8s (invoked by `gen-keys-and-certs.sh`) | Dev | Yes | `projects/k8s/traefik/certs/regenerate_certs.sh` |
| `regenerate_certs.sh` (OTLP) | Certificate generation script (RSA-4096, CA + collector cert) | Dev — rotate OTLP collector TLS certs for the observability stack (invoked by `gen-keys-and-certs.sh`) | Dev | Yes | `projects/observability/certs/regenerate_certs.sh` |
| `hydra-create-client.sh` | Hydra OAuth2 client registration script | Dev — register a client in Hydra with specified scopes; prints env vars for CLI use | Dev | Yes | `scripts/hydra-create-client.sh` |
