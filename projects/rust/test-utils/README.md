# udex-test-utils

Shared test fixture helpers for Udex integration tests.

**Dev-only** — this crate is `publish = false` and must only ever appear as a `[dev-dependency]`. It must never be added to the production dependency tree.

## Exports

| Function | Description |
|---|---|
| `bind_file_secret(path)` | Creates a `Secret<String>` backed by a file path, using `secrets-rs` `FileSource` |
| `hydra_public_url()` | Returns `HYDRA_PUBLIC_URL` env var, defaulting to `http://localhost:4444` |
| `hydra_admin_url()` | Returns `HYDRA_ADMIN_URL` env var, defaulting to `http://localhost:4445` |
| `register_hydra_client(admin_url, client_id, client_secret, audience, scopes)` | Upserts an OAuth2 client in Hydra (create, or replace on 409) |

## Usage

Add to the test crate's `Cargo.toml`:

```toml
[dev-dependencies]
udex-test-utils = { path = "../test-utils" }
```

Then import in test files:

```rust
use udex_test_utils::{bind_file_secret, hydra_admin_url, register_hydra_client};
```

## Environment

`hydra_public_url` and `hydra_admin_url` read from environment variables that are set automatically in the VS Code dev container (pointing at the `hydra` service in the compose stack). No manual configuration is needed when running inside the devcontainer.
