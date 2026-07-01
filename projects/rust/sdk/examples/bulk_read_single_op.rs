//! Read entries back with a **single-operation-type** `bulk_read`.
//!
//! Every operation in the batch is the same kind (`LookupContext`): resolve a
//! context from its entry key. Results come back in input order, one result
//! variant per slot.
//!
//! `bulk_read` needs entries to read, so this example first seeds a few with
//! `create_entry`, then looks them all up by key in one batch.
//!
//! For a batch that mixes both lookup directions (key -> context and
//! context -> key) see [`bulk_read_mixed_ops`](bulk_read_mixed_ops).
//!
//! # Required environment variables
//!
//! | Variable             | Description |
//! |----------------------|-------------|
//! | `UDEX_ENDPOINT`      | gRPC server URL, e.g. `https://localhost:50051` |
//! | `UDEX_TOKEN_URL`     | OAuth2 token endpoint URL |
//! | `UDEX_CLIENT_ID`     | OAuth2 client ID |
//! | `UDEX_CLIENT_SECRET` | OAuth2 client secret |
//! | `UDEX_INDEX`         | Index name to read/write |
//!
//! # Optional environment variables
//!
//! | Variable        | Description |
//! |-----------------|-------------|
//! | `UDEX_CA_CERT`  | **Dev only** — path to a PEM CA certificate for self-signed setups |
//! | `UDEX_AUDIENCE` | `audience` parameter for the token request |
//! | `UDEX_SCOPE`    | Space-separated scopes to request |
//!
//! ```bash
//! cargo run --example bulk_read_single_op
//! ```

use udex_api::entry::{
    bulk_read_entry_operation, bulk_read_entry_operation_result, LookupContextByKeyRequest,
};
use udex_sdk::{
    BulkReadEntryOperation, ClientOptions, ContextInput, KeyValuePair, UdexClient, Value,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv_override().ok();
    tracing_subscriber::fmt::init();

    let (client, index) = connect().await?;

    // Seed a few entries so there is something to read back, keeping their keys.
    let mut keys: Vec<String> = Vec::new();
    for pairs in [
        [("user_id", "1"), ("region", "eu")],
        [("user_id", "2"), ("region", "us")],
        [("user_id", "3"), ("region", "eu")],
    ] {
        let resp = client.create_entry(&index, context(&pairs)).await?;
        keys.push(resp.key);
    }

    // One LookupContext op per key — a single-op-type read batch.
    let operations: Vec<BulkReadEntryOperation> = keys
        .iter()
        .map(|key| BulkReadEntryOperation {
            operation: Some(bulk_read_entry_operation::Operation::LookupContext(
                LookupContextByKeyRequest {
                    index_name: index.clone(),
                    key: key.clone(),
                },
            )),
        })
        .collect();

    let results = client.bulk_read(&index, operations).await?;

    println!("Read {} entries by key:", results.len());
    for (i, result) in results.iter().enumerate() {
        match result
            .result
            .as_ref()
            .ok_or_else(|| format!("result[{i}]: server returned empty result"))?
        {
            bulk_read_entry_operation_result::Result::LookupContext(r) => match &r.context {
                Some(ctx) => println!(
                    "  [{i}] key={} -> hash={} ({} pairs)",
                    keys[i],
                    ctx.hash,
                    ctx.pairs.len()
                ),
                None => println!("  [{i}] key={} -> not found", keys[i]),
            },
            other => println!("  [{i}] unexpected result: {other:?}"),
        }
    }

    Ok(())
}

/// Builds a [`ContextInput`] from flat string key-value pairs.
fn context(pairs: &[(&str, &str)]) -> ContextInput {
    ContextInput {
        pairs: pairs
            .iter()
            .map(|(k, v)| KeyValuePair {
                key: (*k).to_owned(),
                value: Some(Value {
                    value: Some(udex_sdk::value::Value::StringValue((*v).to_owned())),
                }),
                kek_id: None,
                dek: None,
            })
            .collect(),
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
