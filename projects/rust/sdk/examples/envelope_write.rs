//! Create an entry with one envelope-encrypted context value and retrieve it.
//!
//! Demonstrates client-side AES-256-GCM envelope encryption. The server stores
//! and returns all encryption fields verbatim — it never inspects or decrypts
//! them. All encryption and decryption happens here, in the client.
//!
//! The example takes plaintext KEY=VALUE pairs from the command line and
//! one additional pair whose value is encrypted before submission and decrypted
//! after retrieval, identified by `UDEX_ENCRYPTED_KEY` / `UDEX_ENCRYPTED_VALUE`.
//!
//! # Required environment variables
//!
//! | Variable                | Description |
//! |-------------------------|-------------|
//! | `UDEX_ENDPOINT`         | gRPC server URL, e.g. `https://localhost:50051` |
//! | `UDEX_TOKEN_URL`        | OAuth2 token endpoint URL |
//! | `UDEX_CLIENT_ID`        | OAuth2 client ID |
//! | `UDEX_CLIENT_SECRET`    | OAuth2 client secret |
//! | `UDEX_INDEX`            | Index name to write into |
//! | `UDEX_KEK`              | Base64-encoded 32-byte key-encryption key |
//! | `UDEX_KEK_ID`           | Opaque label identifying the KEK (e.g. `my-kek-v1`) |
//! | `UDEX_ENCRYPTED_KEY`    | Context key whose value will be encrypted |
//! | `UDEX_ENCRYPTED_VALUE`  | Plaintext value to encrypt |
//!
//! # Optional environment variables
//!
//! | Variable        | Description |
//! |-----------------|-------------|
//! | `UDEX_CA_CERT`  | **Dev only** — path to a PEM CA certificate for self-signed setups |
//! | `UDEX_AUDIENCE` | `audience` parameter for the token request |
//! | `UDEX_SCOPE`    | Space-separated scopes to request |
//!
//! # Example
//!
//! Generate a one-off KEK, then create an entry with a plaintext `user_id`
//! and an encrypted `email`:
//!
//! ```bash
//! export UDEX_KEK=$(openssl rand -base64 32)
//! export UDEX_KEK_ID=my-kek-v1
//! export UDEX_ENCRYPTED_KEY=email
//! export UDEX_ENCRYPTED_VALUE=alice@example.com
//!
//! cargo run --example envelope_write -- user_id=42 region=eu-west
//! ```
//!
//! # Key rotation warning
//!
//! The server computes the context hash over the submitted ciphertext. If you
//! rotate a KEK and re-encrypt a DEK (or re-encrypt a value directly), the
//! ciphertext changes, the hash changes, and the server creates a **new entry**.
//! The old entry survives under its original key. Plan accordingly: delete the
//! old entry before or after creating the new one, or accept both existing
//! during the transition window.

use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use udex_sdk::{ClientOptions, ContextInput, KeyValuePair, UdexClient, Value};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv_override().ok();
    tracing_subscriber::fmt::init();

    // Load the KEK from the environment.
    let kek_b64 = var("UDEX_KEK")?;
    let kek_bytes = B64.decode(&kek_b64)?;
    if kek_bytes.len() != 32 {
        return Err(format!(
            "UDEX_KEK must be 32 bytes when decoded (got {})",
            kek_bytes.len()
        )
        .into());
    }
    let kek = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&kek_bytes));
    let kek_id = var("UDEX_KEK_ID")?;

    let enc_key = var("UDEX_ENCRYPTED_KEY")?;
    let enc_plaintext = var("UDEX_ENCRYPTED_VALUE")?;

    let (client, index) = connect().await?;

    // ── Encrypt ───────────────────────────────────────────────────────────────

    // Generate a fresh DEK scoped to this context.
    let dek_bytes = Aes256Gcm::generate_key(OsRng);
    let dek = Aes256Gcm::new(&dek_bytes);

    // Encrypt the sensitive value with the DEK.
    // Wire format: base64(nonce || ciphertext).
    let value_nonce = Aes256Gcm::generate_nonce(OsRng);
    let value_ct = dek
        .encrypt(&value_nonce, enc_plaintext.as_bytes())
        .map_err(|_| "failed to encrypt value")?;
    let encrypted_value = B64.encode([value_nonce.as_slice(), &value_ct].concat());

    // Wrap the DEK with the KEK.
    let dek_nonce = Aes256Gcm::generate_nonce(OsRng);
    let dek_ct = kek
        .encrypt(&dek_nonce, dek_bytes.as_slice())
        .map_err(|_| "failed to wrap DEK")?;
    let wrapped_dek = B64.encode([dek_nonce.as_slice(), &dek_ct].concat());

    // ── Build context ─────────────────────────────────────────────────────────

    // Plaintext pairs from KEY=VALUE command-line arguments.
    let mut pairs: Vec<KeyValuePair> = std::env::args()
        .skip(1)
        .map(|s| {
            let (k, v) = s
                .split_once('=')
                .unwrap_or_else(|| panic!("expected KEY=VALUE, got: {s:?}"));
            plaintext_pair(k, v)
        })
        .collect();

    // Append the encrypted pair — kek_id and dek travel with the pair itself.
    pairs.push(KeyValuePair {
        key: enc_key.clone(),
        value: Some(Value {
            value: Some(udex_sdk::value::Value::StringValue(encrypted_value)),
        }),
        kek_id: Some(kek_id.clone()),
        dek: Some(wrapped_dek),
    });

    let ctx = ContextInput { pairs };

    // ── Create ────────────────────────────────────────────────────────────────

    let created = client.create_entry(&index, ctx).await?;
    println!("key:          {}", created.key);
    println!("context_hash: {}", created.context_hash);

    // ── Retrieve and decrypt ──────────────────────────────────────────────────

    let found = client.lookup_context_by_key(&index, &created.key).await?;

    println!("\nContext pairs:");
    for pair in &found.pairs {
        if let (Some(wrapped_dek), Some(_kek_id)) = (pair.dek.as_deref(), pair.kek_id.as_deref()) {
            // Recover the DEK stored on this pair, then decrypt the value.
            let wrapped = B64.decode(wrapped_dek)?;
            let (dek_nonce_bytes, dek_ct_bytes) = wrapped.split_at(12);
            let recovered_dek = kek
                .decrypt(Nonce::from_slice(dek_nonce_bytes), dek_ct_bytes)
                .map_err(|_| "failed to unwrap DEK — wrong KEK or corrupt data")?;
            let dek_dec = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&recovered_dek));

            if let Some(udex_sdk::value::Value::StringValue(enc)) =
                pair.value.as_ref().and_then(|v| v.value.as_ref())
            {
                let enc_bytes = B64.decode(enc)?;
                let (nonce_bytes, ct_bytes) = enc_bytes.split_at(12);
                let decrypted = dek_dec
                    .decrypt(Nonce::from_slice(nonce_bytes), ct_bytes)
                    .map_err(|_| "failed to decrypt value")?;
                println!(
                    "  {} = {} (decrypted)",
                    pair.key,
                    String::from_utf8(decrypted)?
                );
            }
        } else {
            let v = pair
                .value
                .as_ref()
                .and_then(|v| v.value.as_ref())
                .map(|v| format!("{v:?}"))
                .unwrap_or_else(|| "<none>".into());
            println!("  {} = {}", pair.key, v);
        }
    }

    Ok(())
}

fn plaintext_pair(key: &str, value: &str) -> KeyValuePair {
    KeyValuePair {
        key: key.to_owned(),
        value: Some(Value {
            value: Some(udex_sdk::value::Value::StringValue(value.to_owned())),
        }),
        kek_id: None,
        dek: None,
    }
}

async fn connect() -> Result<(UdexClient, String), Box<dyn std::error::Error>> {
    let endpoint = var("UDEX_ENDPOINT")?;
    let token_url = var("UDEX_TOKEN_URL")?;
    let client_id = var("UDEX_CLIENT_ID")?;
    let client_secret = var("UDEX_CLIENT_SECRET")?;
    let index = var("UDEX_INDEX")?;

    let mut builder = ClientOptions::builder()
        .endpoint(endpoint)
        .client_credentials(token_url, client_id, client_secret);

    if let Ok(ca) = std::env::var("UDEX_CA_CERT") {
        builder = builder.ca_cert_pem_file(ca);
    }
    if let Ok(aud) = std::env::var("UDEX_AUDIENCE") {
        builder = builder.audience(aud);
    }
    if let Ok(scope) = std::env::var("UDEX_SCOPE") {
        builder = builder.scope(scope);
    }

    let client = UdexClient::connect(builder.build()?).await?;
    Ok((client, index))
}

fn var(name: &str) -> Result<String, Box<dyn std::error::Error>> {
    std::env::var(name).map_err(|_| format!("required env var {name} is not set").into())
}
