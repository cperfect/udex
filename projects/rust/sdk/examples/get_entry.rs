//! Look up an entry in a Udex index.
//!
//! Demonstrates two lookup directions:
//!
//! - **by key** (`lookup_context_by_key`) — retrieve the context for a known UUID key.
//! - **by context** (`lookup_key_by_context`) — find the key for a set of KEY=VALUE pairs.
//!
//! # Required environment variables
//!
//! | Variable             | Description |
//! |----------------------|-------------|
//! | `UDEX_ENDPOINT`      | gRPC server URL |
//! | `UDEX_TOKEN_URL`     | OAuth2 token endpoint URL |
//! | `UDEX_CLIENT_ID`     | OAuth2 client ID |
//! | `UDEX_CLIENT_SECRET` | OAuth2 client secret |
//! | `UDEX_INDEX`         | Index name to query |
//!
//! # Optional environment variables
//!
//! | Variable        | Description |
//! |-----------------|-------------|
//! | `UDEX_CA_CERT`  | **Dev only** — path to a PEM CA certificate for self-signed setups |
//! | `UDEX_AUDIENCE` | `audience` parameter for the token request |
//! | `UDEX_SCOPE`    | Space-separated scopes to request |
//! | `UDEX_KEY`      | Entry key (UUID) for `lookup_context_by_key` |
//!
//! Run with a key to look up by UUID:
//! ```bash
//! UDEX_KEY=<uuid> cargo run --example get_entry
//! ```
//!
//! Or pass KEY=VALUE pairs as arguments to look up by context:
//! ```bash
//! cargo run --example get_entry -- user_id=42 region=eu-west
//! ```

use udex_api::hash::xxh3_context_hash;
use udex_sdk::{ClientOptions, ContextInput, KeyValuePair, UdexClient, Value};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv_override().ok();
    tracing_subscriber::fmt::init();

    let (client, index) = connect().await?;
    let args: Vec<String> = std::env::args().skip(1).collect();

    if let Ok(key) = std::env::var("UDEX_KEY") {
        // ── Lookup by key ─────────────────────────────────────────────────────
        let ctx = client.lookup_context_by_key(&index, &key).await?;
        println!("context for key {key}:");
        for pair in ctx.pairs {
            let val = pair
                .value
                .and_then(|v| v.value)
                .map(|v| match v {
                    udex_sdk::value::Value::StringValue(s) => s,
                    udex_sdk::value::Value::IntValue(i) => i.to_string(),
                    udex_sdk::value::Value::FloatValue(f) => f.to_string(),
                    udex_sdk::value::Value::BoolValue(b) => b.to_string(),
                })
                .unwrap_or_default();
            println!("  {}={}", pair.key, val);
        }
    } else if !args.is_empty() {
        // ── Lookup by context (KEY=VALUE pairs as CLI args) ───────────────────
        let pairs: Vec<KeyValuePair> = args
            .iter()
            .map(|s| {
                let (k, v) = s
                    .split_once('=')
                    .unwrap_or_else(|| panic!("expected KEY=VALUE, got: {s:?}"));
                KeyValuePair {
                    key: k.to_owned(),
                    value: Some(Value {
                        value: Some(udex_sdk::value::Value::StringValue(v.to_owned())),
                    }),
                    kek_id: None,
                }
            })
            .collect();

        let context_input = ContextInput {
            pairs,
            dek: None,
            kek_id: None,
        };
        let context_hash = xxh3_context_hash(&context_input)?;

        match client
            .lookup_key_by_context(&index, &context_hash)
            .await?
        {
            Some(key) => println!("key: {key}"),
            None => println!("not found"),
        }
    } else {
        eprintln!("Usage:");
        eprintln!("  UDEX_KEY=<uuid> get_entry          # look up by key");
        eprintln!("  get_entry -- KEY=VALUE ...          # look up by context");
    }

    Ok(())
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
