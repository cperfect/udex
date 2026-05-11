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

### Envelope encryption

Udex supports client-side envelope encryption for sensitive context values. The
server treats all encryption fields as opaque blobs — it stores and returns them
verbatim and never inspects, validates, or decrypts them. **Encryption and
decryption are entirely the client's responsibility.**

The pattern uses two keys:

- **KEK (Key Encryption Key)** — a long-lived master key held securely by the
  client (e.g. in a key vault). Identified by a `kek_id` string you choose.
- **DEK (Data Encryption Key)** — a short-lived key generated fresh for each
  context. Encrypted with the KEK and stored alongside the context so that any
  authorised holder of the KEK can recover it.

Individual key-value pairs carry a `kek_id` to signal that their `value` field
contains ciphertext rather than plaintext. Pairs without `kek_id` are stored in
plaintext.

**Wire format.** The SDK uses no built-in encoding for ciphertext. The snippets
below adopt the convention `base64(nonce || ciphertext)` stored as a
`StringValue`, which is compact and self-contained. You may use any format your
application requires, as long as you apply it consistently on read and write.

**Critical: the ciphertext is part of the context identity.** The server hashes
the context exactly as submitted — encrypted values included. If you later
re-encrypt a value (for example because a KEK was rotated and the DEK was
re-wrapped), the ciphertext changes, the context hash changes, and the server
will create a new entry. The old entry remains under its original key and the
old context hash. Plan for this when designing key-rotation procedures: either
accept two entries during the transition, or delete the old entry and create the
new one atomically.

```rust
use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use udex_sdk::{ContextInput, KeyValuePair, Value, value};

// ── Setup ─────────────────────────────────────────────────────────────────────

// KEK: held in a key vault; loaded here from wherever you store it.
// The kek_id is an opaque label the server stores and echoes back.
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
let ctx = ContextInput {
    pairs: vec![
        KeyValuePair {                                          // plaintext
            key: "user_id".into(),
            value: Some(Value { value: Some(value::Value::StringValue("42".into())) }),
            kek_id: None,
        },
        KeyValuePair {                                          // encrypted
            key: "email".into(),
            value: Some(Value { value: Some(value::Value::StringValue(encrypted_value)) }),
            kek_id: Some(kek_id.into()),
        },
    ],
    dek: Some(wrapped_dek),
    kek_id: Some(kek_id.into()),
};

let created = client.create_entry("my-index", ctx).await?;

// ── Decrypt ───────────────────────────────────────────────────────────────────

let found_ctx = client.lookup_context_by_key("my-index", &created.key).await?;

// Unwrap the DEK using the KEK identified by found_ctx.kek_id.
let wrapped = B64.decode(found_ctx.dek.as_deref().unwrap())?;
let (dek_nonce_bytes, dek_ct_bytes) = wrapped.split_at(12);
let recovered_dek = kek.decrypt(Nonce::from_slice(dek_nonce_bytes), dek_ct_bytes)?;
let dek_dec = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&recovered_dek));

// Decrypt each pair whose kek_id is set.
for pair in &found_ctx.pairs {
    if pair.kek_id.is_some() {
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
| [`envelope_write`](examples/envelope_write.rs) | Create an entry with one AES-256-GCM envelope-encrypted value, then retrieve and decrypt it |

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

# Create an entry with one encrypted context value.
export UDEX_KEK=$(openssl rand -base64 32)
export UDEX_KEK_ID=my-kek-v1
export UDEX_ENCRYPTED_KEY=email
export UDEX_ENCRYPTED_VALUE=alice@example.com
cargo run --example envelope_write -- user_id=42 region=eu-west
```
