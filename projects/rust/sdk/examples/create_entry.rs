//! Create an entry in a Udex index.
//!
//! Demonstrates connecting to the server, authenticating via the OAuth2
//! client-credentials flow, and calling `create_entry`.  The auth server is
//! kept opaque — any server that supports client-credentials (Hydra, Keycloak,
//! Auth0, …) works as long as you point the variables at its token endpoint.
//!
//! # Required environment variables
//!
//! | Variable             | Description |
//! |----------------------|-------------|
//! | `UDEX_ENDPOINT`      | gRPC server URL, e.g. `https://localhost:50051` |
//! | `UDEX_TOKEN_URL`     | OAuth2 token endpoint URL |
//! | `UDEX_CLIENT_ID`     | OAuth2 client ID |
//! | `UDEX_CLIENT_SECRET` | OAuth2 client secret |
//! | `UDEX_INDEX`         | Index name to write into |
//!
//! # Optional environment variables
//!
//! | Variable        | Description |
//! |-----------------|-------------|
//! | `UDEX_CA_CERT`  | **Dev only** — path to a PEM CA certificate for self-signed setups |
//! | `UDEX_AUDIENCE` | `audience` parameter for the token request |
//! | `UDEX_SCOPE`    | Space-separated scopes to request |
//!
//! Place these in a `.env` file or export them before running:
//!
//! ```bash
//! cargo run --example create_entry
//! ```

use udex_sdk::{ClientOptions, ContextInput, KeyValuePair, UdexClient, Value};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv_override().ok();
    tracing_subscriber::fmt::init();

    let (client, index) = connect().await?;

    // Build a context from command-line KEY=VALUE arguments, falling back to a
    // hard-coded demo pair when no arguments are supplied.
    let pairs: Vec<KeyValuePair> = if std::env::args().len() > 1 {
        std::env::args()
            .skip(1)
            .map(|s| {
                let (k, v) = s
                    .split_once('=')
                    .unwrap_or_else(|| panic!("expected KEY=VALUE, got: {s:?}"));
                kv(k, v)
            })
            .collect()
    } else {
        vec![kv("example_key", "example_value")]
    };

    let resp = client
        .create_entry(
            &index,
            ContextInput {
                pairs,
                dek: None,
                kek_id: None,
            },
        )
        .await?;

    println!("key:          {}", resp.key);
    println!("context_hash: {}", resp.context_hash);
    Ok(())
}

fn kv(key: &str, value: &str) -> KeyValuePair {
    KeyValuePair {
        key: key.to_owned(),
        value: Some(Value {
            value: Some(udex_sdk::value::Value::StringValue(value.to_owned())),
        }),
        kek_id: None,
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
