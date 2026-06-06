# Security Policy

## Supported Versions

Udex is currently in early development. Only the latest commit on `main` receives security fixes.

| Version | Supported |
|---------|-----------|
| `main` (latest) | ✅ |
| older commits | ❌ |

## Reporting a Vulnerability

**Please do not report security vulnerabilities through public GitHub issues.**

Use [GitHub Private Security Advisories](https://github.com/cperfect/udex/security/advisories/new) to report vulnerabilities confidentially. This keeps details private until a fix is available.

Include as much of the following as you can:

- Type of vulnerability (e.g. authentication bypass, injection, information disclosure)
- The component affected (server, CLI, API crate, datastore)
- File paths and line numbers relevant to the issue
- Step-by-step instructions to reproduce
- Proof-of-concept or exploit code (if available)
- Impact assessment — what an attacker could achieve

## Response Timeline

Response times are best efforts only.

| Milestone | Target |
|-----------|--------|
| Acknowledgement | 3 business days |
| Initial assessment | 7 business days |
| Fix or workaround | Dependent on severity and complexity |

## Disclosure Policy

Once a fix is available and released, the vulnerability will be disclosed via a GitHub Security Advisory. Credit will be given to the reporter unless they prefer to remain anonymous.

## Secrets Management

### Injection model

Secret values are never stored in committed files. Instead, config files reference secrets
by URN and the server resolves them at startup via the
[`secrets-rs`](https://crates.io/crates/secrets-rs) crate.

**Environment variable source** — used for string secrets such as database URLs:

```yaml
datastore:
  connection_url: "urn:secrets-rs:env:DATABASE_URL"
```

`UdexConfig::load()` calls `bind_all()` after deserialisation; startup fails with a clear
error if the named environment variable is absent.

**File source** — used for multi-line PEM material (TLS certificate, TLS private key, JWT
public key). Paths are resolved relative to the config file's directory:

```yaml
server:
  tls:
    cert: "urn:secrets-rs:file:certs/server.crt"
    key: "urn:secrets-rs:file:certs/server.key"
  authz:
    jwt_public_key: "urn:secrets-rs:file:certs/jwt_public_key.pem"
```

Both URN types produce a `Secret<String>` whose value is masked in all `Debug`/`Display`
output, preventing accidental logging of key material.

For development, run `scripts/gen-env.sh` to generate `.env` with all required values.
The devcontainer runs this automatically on first start.

### File-injection guard

`Secret<T>` from `secrets-rs` enforces the URN contract at the serde layer:
`Secret<T>::Deserialize` only accepts a valid URN string. A config file containing a
raw secret value (e.g. `connection_url: "postgres://user:pass@host/db"`) is rejected
at YAML parse time with a clear error — it never reaches application code.

### CLI argument restriction

Bearer tokens and OAuth2 client secrets are accepted from environment variables only.
The corresponding CLI flags (`--token`, `--client-secret`) do not exist, so secret
values cannot appear in process listings or shell history.

| Secret | Environment variable |
|--------|----------------------|
| Bearer token for gRPC calls | `UDEX_TOKEN` |
| OAuth2 client secret (`token fetch`) | `UDEX_CLIENT_SECRET` |

### Secret inventory

See [`SECRETS.md`](docs/SECRETS.md) for the full inventory of credentials, keys,
certificates, and their sources.

## Scope

The following are **in scope**:

- `udex-server` — gRPC server, authentication, authorisation
- `udex-api` — protobuf types, JWT claims, authorisation logic
- `udex-datastore` — PostgreSQL datastore implementation
- `udex` CLI — token handling, configuration, server communication

The following are **out of scope**:

- Vulnerabilities in third-party dependencies (report these upstream; see `.trivyignore` for acknowledged findings with rationale)
- Issues requiring physical access to the host
- Social engineering attacks
