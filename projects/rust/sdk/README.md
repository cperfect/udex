# udex-sdk

Rust client library for [Udex](../../README.md).

Provides an idiomatic async Rust API over the Udex gRPC service: TLS channel
construction, transparent OAuth2 client-credentials token management, and
strongly-typed wrappers for every entry and index operation.

## Getting started

Add the crate to your `Cargo.toml`:

```toml
[dependencies]
udex-sdk = "0.1.0"
tokio    = { version = "1", features = ["macros", "rt-multi-thread"] }
```

For local development against an unreleased checkout, use a path dependency
relative to your own project:

```toml
udex-sdk = { path = "../udex/projects/rust/sdk" }
```

### Connect and authenticate

```rust
use udex_sdk::{ClientOptions, UdexClient};

let client = UdexClient::connect(
    ClientOptions::builder()
        .endpoint("https://udex.example.com:50051")
        .client_credentials(
            "https://auth.example.com/oauth2/token",
            "my-client-id",
            "my-client-secret",
        )
        .build()?,
)
.await?;
```

The SDK works with any server that supports the OAuth2
[client-credentials flow](https://datatracker.ietf.org/doc/html/rfc6749#section-4.4)
(Hydra, Keycloak, Auth0, …). Tokens are fetched on first use and refreshed
automatically. The specific auth server is opaque to the SDK.

#### Optional token request parameters

```rust
ClientOptions::builder()
    // …
    .client_credentials(token_url, client_id, client_secret)
    .audience("https://api.example.com")   // RFC 8693 audience
    .scope("udex:entry:v1:my-index:read")  // space-separated scopes
    .build()?
```

#### Development only — custom CA certificate

In production the SDK uses the system trust store. For local development
environments that use a self-signed or private CA certificate:

```rust
// Development only — use only with dev/test deployments.
ClientOptions::builder()
    .endpoint("https://localhost:50051")
    .ca_cert_pem_file("/path/to/dev-ca.pem")
    .client_credentials(token_url, client_id, client_secret)
    .build()?
```

#### Development only — static bearer token

For unit tests or scripts where you want to bypass the auth server entirely:

```rust
// Development only — avoid in production workloads.
ClientOptions::builder()
    .endpoint("https://localhost:50051")
    .static_bearer_token(token_string)
    .build()?
```

#### Development only — plain HTTP endpoints

By default the SDK enforces TLS for both the gRPC endpoint and the OAuth2
token URL: `build()` returns an error if either starts with `http://`, and the
underlying transports (tonic channel and reqwest HTTP client) also enforce TLS
independently.

For local dev environments where the server or auth service does not have TLS
(e.g. a Hydra instance running on `http://localhost:4444`), call
`danger_allow_non_tls()` to opt out:

```rust
// Development only — never use against a production environment.
ClientOptions::builder()
    .endpoint("http://localhost:50051")
    .client_credentials(
        "http://localhost:4444/oauth2/token",
        "my-client-id",
        "my-client-secret",
    )
    .danger_allow_non_tls()
    .build()?
```

## Entry operations

```rust
use udex_sdk::{xxh3_context_hash, ContextInput, KeyValuePair, Value, value};

// Build a context — a set of key-value pairs that identify an entity.
fn kv(key: &str, val: &str) -> KeyValuePair {
    KeyValuePair {
        key: key.into(),
        value: Some(Value {
            value: Some(value::Value::StringValue(val.into())),
        }),
        kek_id: None,
    }
}

let context_input = ContextInput {
    pairs: vec![kv("user_id", "42"), kv("region", "eu")],
    dek: None,
    kek_id: None,
};

// Pre-compute the hash before context_input is consumed by create_entry.
let hash = xxh3_context_hash(&context_input)?;

// Create (idempotent — returns the existing entry if context already exists).
let resp = client.create_entry("my-index", context_input).await?;
println!("key: {}", resp.key);

// Look up context for a known key (UUID).
let ctx = client.lookup_context_by_key("my-index", &resp.key).await?;

// Reverse lookup: find the key for a context hash.
let key: Option<String> = client.lookup_key_by_context("my-index", &hash).await?;

// Delete an entry.
client.delete_entry("my-index", &resp.key).await?;
```

### Bulk operations

```rust
use udex_sdk::{BulkWriteEntryOperation, CreateEntryRequest, bulk_write_entry_operation};

let operations = vec![
    BulkWriteEntryOperation {
        operation: Some(bulk_write_entry_operation::Operation::CreateEntry(
            CreateEntryRequest {
                index_name: "my-index".into(),
                context: Some(ctx_a),
            },
        )),
    },
    // …
];

let results = client.bulk_write("my-index", operations).await?;
```

## Index operations

```rust
use udex_sdk::CreateIndexRequest;

// List all indices.
let indices = client.list_indices().await?;

// Describe a single index.
let idx = client.describe_index("my-index").await?;

// Create an index.
client.create_index(CreateIndexRequest { name: "my-index".into(), ..Default::default() }).await?;
```

## Environment variable convention

All runnable examples read their configuration from environment variables.
Place these in a `.env` file (loaded via `dotenvy`) or export them:

| Variable             | Required | Description                                  |
|----------------------|----------|----------------------------------------------|
| `UDEX_ENDPOINT`      | yes      | gRPC server URL, e.g. `https://udex.example.com:50051` |
| `UDEX_TOKEN_URL`     | yes      | OAuth2 token endpoint URL                    |
| `UDEX_CLIENT_ID`     | yes      | OAuth2 client ID                             |
| `UDEX_CLIENT_SECRET` | yes      | OAuth2 client secret                         |
| `UDEX_INDEX`         | yes      | Index name to operate on                     |
| `UDEX_CA_CERT`       | no       | **Dev only** — path to a PEM CA certificate for self-signed setups |
| `UDEX_AUDIENCE`      | no       | `audience` parameter for the token request   |
| `UDEX_SCOPE`         | no       | Space-separated scopes to request            |

## Examples

| Example | Description |
|---------|-------------|
| [`create_entry`](examples/create_entry.rs) | Connect, authenticate, and create an entry from CLI `KEY=VALUE` arguments |
| [`get_entry`](examples/get_entry.rs) | Look up an entry by UUID key or by context (`KEY=VALUE` arguments) |
| [`bulk_write`](examples/bulk_write.rs) | Batch-create entries from newline-delimited JSON on stdin |

Run any example with the environment variables set:

```bash
# Create an entry with two context pairs.
cargo run --example create_entry -- user_id=42 region=eu-west

# Look up by key.
UDEX_KEY=<uuid> cargo run --example get_entry

# Look up by context.
cargo run --example get_entry -- user_id=42 region=eu-west

# Bulk-create entries from JSON.
printf '{"user_id":"1","region":"eu"}\n{"user_id":"2","region":"us"}\n' |
    cargo run --example bulk_write
```
