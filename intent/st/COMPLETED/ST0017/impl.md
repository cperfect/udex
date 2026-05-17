# Implementation - ST0017: Integration test consolidation and shared test utilities

## Implementation

ST0017 delivered four work packages that cleaned up the integration test estate and established a durable shared-fixture pattern. The work was purely structural — no production code changed.

### WP-01: `udex-test-utils` crate

A new dev-only workspace crate was created at `projects/rust/test-utils/` to consolidate the fixture helpers that had been copy-pasted across `sdk/tests/`, `server/tests/`, and `cli/tests/`. The crate exports:

- `bind_file_secret(path)` — wraps `secrets-rs` `FileSource` to create a `Secret<String>` from a file path
- `hydra_public_url()` — reads `HYDRA_PUBLIC_URL` env var with a `localhost:4444` fallback
- `hydra_admin_url()` — reads `HYDRA_ADMIN_URL` env var with a `localhost:4445` fallback
- `register_hydra_client(admin_url, client_id, client_secret, audience, scopes)` — upserts a Hydra OAuth2 client (create or replace on 409)

The crate is marked `publish = false` and is a `[dev-dependency]` only. It must never appear in the production dependency tree.

One non-obvious implementation detail: `SourceRegistry` lives at the `secrets_rs` crate root, not in `secrets_rs::sources` — using the wrong path causes a compile error.

The `register_hydra_client` signature differs from the old local copies: it takes an explicit `scopes: &str` parameter rather than deriving scopes from an `index_name`. All call sites must inline the scopes string. This was intentional — the function is more general and the scopes needed at each call site vary.

### WP-02: Test function renaming and fixture import consolidation

All test functions across six files were renamed to follow the canonical layer-prefix convention. The `test_hydra_` prefix was replaced with `test_*_oauth2_` because `hydra` is an implementation detail (Ory Hydra is the dev environment OAuth2 server, not a conceptual layer). Tests are named for what they test, not the tool they use.

The `auth_server.rs` module inside `server/tests/` was intentionally kept in place rather than migrated into `udex-test-utils`. It contains a more complex `OAuthClientConfig` struct and an `authenticate` function that uses the `oauth2` crate — functionally distinct from the simple fixture helpers that were migrated.

### WP-03: Slimming service-layer integration tests

The entry service and index service test files were reduced to remove tests fully covered by the SDK suite.

**Why the SDK tests are primary**: The SDK tests exercise TLS termination, JWT validation, gRPC wire format, and the full handler chain in a single pass — the closest approximation of a real client. If the SDK tests pass, the stack works. Service-layer tests bypass TLS and auth, so they can only confirm handler-level logic.

**Why three entry service tests were retained despite the original plan to remove them**: A coverage review (triggered by the user before finalising) identified three tests with unique coverage paths the SDK cannot exercise:

1. `test_entry_service_error_handling` — invalid UUID format rejection. The SDK never sends a non-UUID key.
2. `test_entry_service_lookup_or_create_validation_errors` — missing context, empty `context_hash`, empty `index_name`. The SDK validates these before sending.
3. `test_entry_service_lookup_or_create_hash_mismatch` — server-side rejection of a hash that does not match the server's own computation. The SDK computes the hash correctly before sending; this path is structurally unreachable at the SDK level.

Final counts: entry service — 8 tests; index service — 18 validation-only tests.

### WP-04: Documentation

- `docs/ARCHITECTURE.md` — "Test Strategy" section added: suite hierarchy table, rationale, naming convention, and pointer to `udex-test-utils`
- `CONTRIBUTING.md` (root) — testing section updated with naming convention rule and link to ARCHITECTURE.md
- `projects/rust/CONTRIBUTING.md` — already updated in WP-02 with the canonical naming convention and `udex-test-utils` reference
- `intent/llm/MODULES.md` — `udex-test-utils` section added in WP-01

## Code Examples

### `udex-test-utils` registration call (with explicit scopes)

```rust
register_hydra_client(
    &hydra_admin_url(),
    "my-client",
    "my-secret",
    "https://my-server/",
    &format!("udex:index:v1:list udex:index:v1:{index_name}:read udex:index:v1:{index_name}:write"),
).await;
```

## Technical Details

- `udex-test-utils` uses `ory-hydra-client = { version = "26.2", features = ["rustls-tls"] }` pinned to the same Hydra version as the devcontainer compose stack
- `maybe_once` / `tokio-shared-rt` pattern is used in service-layer tests to share a single Postgres container across all tests in a file, avoiding redundant DB spin-up overhead
- `cargo test --all-targets` is required (not just `cargo test`) — the `--all-targets` flag includes benchmark compilation checks that CI also runs

## Challenges & Solutions

**`SourceRegistry` import path** — initial implementation used `secrets_rs::sources::SourceRegistry` which doesn't exist; the correct path is `secrets_rs::SourceRegistry`. Caught at compile time.

**Duplicate `loc_context` helper** — the Python-based test restoration script accidentally inserted a second copy of the `loc_context` helper function alongside the first. Caught by `cargo fmt --check` (unexpected item after doc comment) and fixed by removing the duplicate block.

**Stale doc comment on `test_entry_service_bulk_write_empty_invalid_argument`** — the comment described a different test (lookup-by-context idempotency). Fixed by replacing it with the correct one-line doc comment.

**`HealthCheck` import in entry service tests** — `clippy` occasionally reported it as unused (stale cache), but removing it causes `error[E0599]: no method named 'is_healthy'` because `HealthCheck` is the trait that provides the method on `EntryService<PostgresDatastore>`. The import is required.
