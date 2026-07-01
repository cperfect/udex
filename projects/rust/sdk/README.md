# udex-sdk

Rust client library for [Udex](../../README.md).

Provides an idiomatic async Rust API over the Udex gRPC service: TLS channel
construction, transparent OAuth2 client-credentials token management, and
strongly-typed wrappers for every entry and index operation.

## One dependency

`udex-sdk` is designed to be a client's **only** Udex dependency. It re-exports
every proto type needed to build requests and read responses — `ContextInput`,
`KeyValuePair`, `Value`, `CreateEntryRequest`, `CreateIndexRequest`,
`HashAlgorithm`, the bulk-operation enums and result types, `xxh3_context_hash`,
and so on — so application and test code import them from `udex_sdk` and never
reach into `udex-api` directly. `udex-api` is an internal crate (generated types,
authz, hashing); depending on it from client code couples you to Udex internals.
If a client operation needs a type that `udex_sdk` does not re-export, that is a
missing re-export in the SDK — please file it rather than importing `udex-api`.

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
`dangerous_allow_non_tls()` to opt out:

```rust
// Development only — never use against a production environment.
ClientOptions::builder()
    .endpoint("http://localhost:50051")
    .client_credentials(
        "http://localhost:4444/oauth2/token",
        "my-client-id",
        "my-client-secret",
    )
    .dangerous_allow_non_tls()
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
        dek: None,
    }
}

let context_input = ContextInput {
    pairs: vec![kv("user_id", "42"), kv("region", "eu")],
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

### Envelope encryption

Udex supports client-side envelope encryption for sensitive context values. The
server treats all encryption fields as opaque blobs — it stores and returns them
verbatim and never inspects, validates, or decrypts them. **Encryption and
decryption are entirely the client's responsibility.**

The pattern uses two keys:

- **KEK (Key Encryption Key)** — a long-lived master key held securely by the
  client (e.g. in a key vault). Identified by a `kek_id` string you choose.
- **DEK (Data Encryption Key)** — a short-lived key generated fresh per context
  (or per pair). Encrypted with the KEK and stored on each encrypted pair so
  that any authorised holder of the KEK can recover it.

Encryption is **per pair**: each `KeyValuePair` carries its own `kek_id` and
`dek`. Pairs without these fields are stored as plaintext. You can use one DEK
for all encrypted pairs in a context (wrapping it on each pair) or generate a
separate DEK per pair.

**Wire format.** The SDK uses no built-in encoding for ciphertext. The snippets
below adopt the convention `base64(nonce || ciphertext)` stored as a
`StringValue`. You may use any format as long as you apply it consistently.

**The ciphertext is part of the context identity.** The server hashes each pair
by `(key, value)` only — `kek_id` and `dek` are not included (they are metadata
and the ciphertext itself already encodes any encryption change). Re-encrypting a
value produces a different ciphertext, a different hash, and a new entry. The old
entry remains under its original key. Plan key-rotation accordingly: delete the
old entry before or after creating the new one, or accept both existing during
the transition window.

```rust
use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use udex_sdk::{ContextInput, KeyValuePair, Value, value};

// ── Setup ─────────────────────────────────────────────────────────────────────

// KEK: held in a key vault; loaded here from wherever you store it.
let kek_id = "my-kek-v1";
let kek_bytes: [u8; 32] = /* load from vault */;
let kek = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&kek_bytes));

// ── Encrypt ───────────────────────────────────────────────────────────────────

// Generate a fresh DEK for this context.
let dek_bytes = Aes256Gcm::generate_key(OsRng);
let dek = Aes256Gcm::new(&dek_bytes);

// Encrypt the sensitive value with the DEK.
// Wire format: base64(nonce || ciphertext).
let plaintext = "alice@example.com";
let value_nonce = Aes256Gcm::generate_nonce(OsRng);
let value_ct = dek.encrypt(&value_nonce, plaintext.as_bytes())?;
let encrypted_value = B64.encode([value_nonce.as_slice(), &value_ct].concat());

// Wrap the DEK with the KEK (same wire format).
let dek_nonce = Aes256Gcm::generate_nonce(OsRng);
let dek_ct = kek.encrypt(&dek_nonce, dek_bytes.as_slice())?;
let wrapped_dek = B64.encode([dek_nonce.as_slice(), &dek_ct].concat());

// Build the context: mix plaintext and encrypted pairs freely.
// kek_id and dek travel with each encrypted pair.
let ctx = ContextInput {
    pairs: vec![
        KeyValuePair {                                          // plaintext
            key: "user_id".into(),
            value: Some(Value { value: Some(value::Value::StringValue("42".into())) }),
            kek_id: None,
            dek: None,
        },
        KeyValuePair {                                          // encrypted
            key: "email".into(),
            value: Some(Value { value: Some(value::Value::StringValue(encrypted_value)) }),
            kek_id: Some(kek_id.into()),
            dek: Some(wrapped_dek),
        },
    ],
};

let created = client.create_entry("my-index", ctx).await?;

// ── Decrypt ───────────────────────────────────────────────────────────────────

let found_ctx = client.lookup_context_by_key("my-index", &created.key).await?;

// Decrypt each pair that carries a dek — unwrap the DEK then decrypt the value.
for pair in &found_ctx.pairs {
    if let (Some(wrapped), Some(_kek_id)) = (pair.dek.as_deref(), pair.kek_id.as_deref()) {
        let wrapped_bytes = B64.decode(wrapped)?;
        let (dek_nonce_bytes, dek_ct_bytes) = wrapped_bytes.split_at(12);
        let recovered_dek = kek.decrypt(Nonce::from_slice(dek_nonce_bytes), dek_ct_bytes)?;
        let dek_dec = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&recovered_dek));

        if let Some(value::Value::StringValue(enc)) = pair.value.as_ref()
            .and_then(|v| v.value.as_ref())
        {
            let enc_bytes = B64.decode(enc)?;
            let (nonce_bytes, ct_bytes) = enc_bytes.split_at(12);
            let decrypted = dek_dec.decrypt(Nonce::from_slice(nonce_bytes), ct_bytes)?;
            println!("{}: {}", pair.key, String::from_utf8(decrypted)?);
        }
    }
}
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

// Delete an index (the index must have no entries).
client.delete_index("my-index").await?;
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
| [`bulk_write_single_op`](examples/bulk_write_single_op.rs) | Batch of one operation type: create several entries in a single transaction |
| [`bulk_write_mixed_ops`](examples/bulk_write_mixed_ops.rs) | Mixed operation types in one transaction: create + lookup-or-create (client-computed hash) + delete |
| [`bulk_read_single_op`](examples/bulk_read_single_op.rs) | Batch of one operation type: read several entries back by key in a single call |
| [`bulk_read_mixed_ops`](examples/bulk_read_mixed_ops.rs) | Mixed lookup directions in one call: key -> context and context -> key (client-computed hash) |
| [`envelope_write`](examples/envelope_write.rs) | Create an entry with one AES-256-GCM envelope-encrypted value, then retrieve and decrypt it |
| [`delete_index`](examples/delete_index.rs) | Delete an empty index, with clear messages if it still has entries or is not found |

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

# Bulk-write with a single operation type (several creates in one transaction).
cargo run --example bulk_write_single_op

# Bulk-write mixing operation types (create + lookup-or-create + delete).
cargo run --example bulk_write_mixed_ops

# Bulk-read with a single operation type (read several entries back by key).
cargo run --example bulk_read_single_op

# Bulk-read mixing lookup directions (key -> context and context -> key).
cargo run --example bulk_read_mixed_ops

# Create an entry with one encrypted context value.
export UDEX_KEK=$(openssl rand -base64 32)
export UDEX_KEK_ID=my-kek-v1
export UDEX_ENCRYPTED_KEY=email
export UDEX_ENCRYPTED_VALUE=alice@example.com
cargo run --example envelope_write -- user_id=42 region=eu-west

# Delete an empty index.
UDEX_INDEX=my-index cargo run --example delete_index
```
