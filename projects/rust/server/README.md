# udex-server

gRPC server crate for Udex. Implements the `IndexService` and `EntryService` handlers, JWT validation and authorization middleware, TLS transport, structured logging, and configuration validation.

## Crate layout

| Module | Responsibility |
|--------|---------------|
| `server.rs` | `start()` (production entry point) and `serve<D>()` (test-friendly generic) |
| `index.rs` | `IndexService` gRPC handler |
| `entry.rs` | `EntryService` gRPC handler |
| `authz.rs` | `AuthzInterceptor` — JWT validation and permission enforcement on every request |
| `config.rs` | `ServerConfig` / `AuthzConfig` with `validate()` |
| `logging.rs` | `init_tracing()` — JSON structured logs via `tracing-subscriber` |

## Running the server

The server is started via the `udex` CLI:

```bash
udex config init          # generate udex.yaml
udex serve                # start the server
```

Or programmatically (e.g. in integration tests):

```rust
udex_server::serve(server_config, datastore).await?;
```

## Authorization

The server enforces authorization on every gRPC request except `grpc.health.v1.Health`.
Authentication — establishing *who* the caller is — is the responsibility of an
upstream OAuth2 server (or a self-signed key pair in development/test).

### How it works

Every request must carry an `Authorization: Bearer <jwt>` header. The
`AuthzInterceptor` validates the token in three steps:

1. **Signature** — The JWT is verified using the configured key source (see
   below). EC P-256 / ES256 is the only supported algorithm.
2. **Standard claims** — `iss`, `aud`, `exp`, and `iat` are checked against
   the values in `authz.*` config.
3. **Permissions** — `udex:` scope values from the RFC 8693 `scope` claim are
   extracted and matched against the required permission for the requested
   operation. Non-`udex:` scopes (e.g. `openid`, `profile`) are silently
   discarded. See [JWT Claims](../api/README.md#jwt-claims) for the full token
   structure and scope format.

Requests that fail signature or claims validation receive `UNAUTHENTICATED`.
Requests with a valid token but insufficient scope receive `PERMISSION_DENIED`.

### Key source: static PEM file

Suitable for development and test environments where tokens are self-signed (e.g.
by the integration test suite or `cargo test`).

```yaml
server:
  authz:
    jwt_public_key: "urn:secrets-rs:file:certs/signing_public_key.pem"  # EC P-256 public key, PEM format
    jwt_issuer: "https://auth.example.com"
    jwt_audience: "udex"
```

The key is loaded once at startup (resolved relative to the config file directory).
To rotate the key the server must be restarted.

### Key source: JWKS endpoint

Suitable for production and for development against a real OAuth2 server (see
[Development with Hydra](#development-with-hydra) below).

At startup the server fetches the JWKS document, builds an in-memory
`kid → DecodingKey` map, and selects the correct key for each incoming token
by reading the `kid` JWT header claim. The cache is then kept fresh by two
complementary mechanisms:

**Cache-miss refresh** — when a token arrives with a `kid` that is not in the
cache (e.g. after key rotation), the server fetches the JWKS endpoint inline
before returning an error. If the new key is found after the refresh, the
request succeeds without any client retry.

**Configured-expiry refresh** — a background task proactively re-fetches the
JWKS on a configurable schedule (default: once per day) so that the cache
stays fresh even during quiet periods.

Both mechanisms share the same fetch path and DoS controls:

- **Max failed refreshes** (`jwks_max_failed_refreshes`, default `5`) — after
  this many consecutive failures the server stops attempting refreshes until
  restart. Cached keys remain valid; only tokens with an unknown `kid` are
  rejected.
- **Exponential backoff with equal jitter** (`jwks_backoff_factor_secs`,
  default `3`) — successive failures back off exponentially
  (`factor^attempt`, capped at 300 s) with a random jitter applied to avoid
  thundering-herd behaviour across a fleet.

```yaml
server:
  authz:
    jwks_url: "http://localhost:4444/.well-known/jwks.json"
    jwt_issuer: "http://localhost:4444/"   # must match Hydra's URLS_SELF_ISSUER
    jwt_audience: "udex"

    # Optional — shown with their defaults:
    # jwks_max_age_secs: 86400  # 1 day; set to 0 to disable expiry refresh
    # jwks_max_failed_refreshes: 5
    # jwks_backoff_factor_secs: 3
```

Exactly one of `jwt_public_key` and `jwks_url` must be set; providing neither
or both is a configuration error caught at startup.

### Development with Hydra

[Ory Hydra](https://www.ory.sh/hydra/) v26.2.0 is included in the Docker Compose
stack and is the reference OAuth2 server for development. To obtain a token:

```bash
# 1. Register an OAuth2 client (once per Hydra instance)
#    Scopes must match the permissions required by your requests.
#    In the devcontainer Hydra is reachable at hydra:4445 (admin) / hydra:4444 (public).
hydra create client \
  --endpoint http://hydra:4445 \
  --id my-client \
  --secret my-secret \
  --grant-type client_credentials \
  --token-endpoint-auth-method client_secret_post \
  --audience udex \
  --scope "udex:index:v1:my-index:read udex:entry:v1:my-index:create"

# 2. Fetch a token using the CLI
udex token fetch \
  --client-id my-client \
  --client-secret my-secret \
  --url http://hydra:4444/oauth2/token \
  --scope udex:index:v1:my-index:read

# 3. Use the token (or set UDEX_TOKEN in your shell)
udex --token eyJ... index get my-index
```

The `udex token fetch` command prints both the raw encoded JWT and the decoded
header and claims, making it easy to verify the token contents before use.

## Configuration

`ServerConfig` is loaded from a YAML file by the CLI. Key fields:

- `bind_address` — socket address (e.g. `127.0.0.1:50051`)
- `tls.cert` / `tls.key` — `urn:secrets-rs:file:` URNs for the TLS certificate and private key (resolved relative to the config file directory)
- `authz.jwt_public_key` or `authz.jwks_url` — JWT key source; see [Authorization](#authorization) above
- `authz.jwt_issuer` / `authz.jwt_audience` — expected `iss` and `aud` claims
- `authz.jwks_max_age_secs` — (JWKS only) cache lifetime in seconds before a proactive background refresh; default `86400`; `0` disables
- `authz.jwks_max_failed_refreshes` — (JWKS only) consecutive failures before refresh is suspended until restart; default `5`
- `authz.jwks_backoff_factor_secs` — (JWKS only) exponential backoff base multiplier between failed refreshes; default `3`
- `init_indexes` — list of indexes to ensure exist on startup

## Testing

Unit tests and integration tests live in `tests/server_integration_tests.rs`. Integration tests require `DATABASE_URL` to be set (provided automatically by the dev container).

```bash
cargo test -p udex-server
```

Integration tests use `RUST_LOG` filtering and run with a real PostgreSQL instance — no mocks.

### Hydra integration tests

Tests with the `_oauth2_` infix (e.g. `test_server_oauth2_*`, `test_sdk_oauth2_*`) require a live Hydra instance.

In the devcontainer `HYDRA_PUBLIC_URL` and `HYDRA_ADMIN_URL` are set
automatically by the devcontainer compose file (pointing at the `hydra` Docker
service). Just run `cargo test` — no prefix needed.

Outside the devcontainer the variables default to `localhost:444x`, which is
correct if you are running Hydra locally or in CI (where Docker publishes the
Hydra ports to the host).

### JWKS refresh integration tests

`tests/jwks_refresh_tests.rs` tests the cache-miss and DoS-control paths
against a live Hydra instance. Each test creates a dedicated Hydra key set
(`/admin/keys/udex-jwks-refresh-{uuid}`) so it is fully isolated from the
main JWKS tests. Hydra is assumed to be running; these tests fail hard if it
is unreachable — they are never skipped.
