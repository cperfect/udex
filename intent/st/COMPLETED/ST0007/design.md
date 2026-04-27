# Design - ST0007: Integrated OAuth2 Authorization Server

## Approach

Extend `AuthnInterceptor` to support a `kid`-keyed map of `DecodingKey`s
populated at startup from a JWKS endpoint, while keeping the existing static
PEM path unchanged. The two paths are mutually exclusive and selected purely
by config. No code outside `udex-server` changes.

Test coverage splits into:
- Unit tests in `authn.rs` (existing, unchanged for PEM path).
- Integration tests in `server_integration_tests.rs` parameterised over two
  server fixtures — one using static PEM, one using Hydra JWKS.
- A test-only helper module `tests/auth_server.rs` handles all Hydra admin API
  calls (client creation) and the `client_credentials` token exchange.

## Design Decisions

### JWKS fetched at startup, not per-request

Fetching the JWKS on every token validation would add latency and a network
dependency per request. Fetching once at startup and building an in-memory
`HashMap<String, DecodingKey>` keeps validation synchronous and fast. Key
rotation (JWKS refresh) is explicitly out of scope.

### `kid`-based key selection

The JWKS spec (RFC 7517) identifies keys by `kid`. The JWT header carries the
matching `kid`. `jsonwebtoken` exposes `JwkSet` and per-`Jwk` key construction.
The interceptor decodes the header (without signature verification) to read
`kid`, then looks it up in the map. If the `kid` is missing or unknown, the
token is rejected with `UNAUTHENTICATED`.

### No Hydra dependency in production code

All Hydra-specific logic (admin API calls via `ory-hydra-client`) lives in
`tests/auth_server.rs` under `[dev-dependencies]`. The server binary depends
only on `jsonwebtoken` (already present) and a minimal HTTP client for the
JWKS fetch at startup.

### Integration test modes

The Hydra-JWKS test fixture is conditionally activated. When
`HYDRA_ADMIN_URL` (and optionally `HYDRA_PUBLIC_URL`) env vars are set, the
test suite spins up a second server fixture that uses a Hydra-issued JWKS URL
and creates real OAuth2 clients. When the env vars are absent the Hydra tests
are skipped cleanly (not failed). Static-PEM tests always run.

### Scope sub-set testing without extra clients

A single Hydra client is created with the full set of scopes needed across all
tests. Individual test cases call `authenticate` requesting only the scopes
they need — Hydra's `client_credentials` endpoint issues a token scoped to
the intersection, so no per-test client provisioning is required.

## Architecture

```text
startup
  AuthnInterceptor::new(config)
    ├── jwt_public_key_path  →  read PEM  →  single DecodingKey
    └── jwks_url             →  HTTP GET  →  JwkSet  →  HashMap<kid, DecodingKey>

per-request
  intercept(req)
    ├── extract Bearer token
    ├── decode header (no verify) → read kid
    ├── look up DecodingKey (map[kid] or the single static key)
    └── decode + validate claims  →  insert into extensions
```

## Alternatives Considered

**Background JWKS refresh task** — rejected; key rotation is out of scope and
adds concurrency complexity (`Arc<RwLock<…>>`) for no benefit right now.

**reqwest as the JWKS HTTP client** — `oauth2` crate already brings `reqwest`
as a dev-dependency; reusing it was considered. A direct blocking `reqwest`
call at startup keeps the production binary lean and avoids pulling the full
`oauth2` crate into non-test deps. Revisit if async JWKS refresh is needed.

**Separate integration test binary for Hydra tests** — rejected; using a
conditional fixture inside the existing test binary keeps the setup simpler and
avoids duplicating server-startup boilerplate.
