//! Batch-create entries with a **single-operation-type** `bulk_write`.
//!
//! Every operation in the batch is the same kind (`CreateEntry`), which is the
//! most common bulk pattern: write many entries in one transaction. The whole
//! batch commits or rolls back together, and results come back in input order.
//!
//! For a mixed batch (create + lookup-or-create + delete in one call) see
//! [`bulk_write_mixed_ops`](bulk_write_mixed_ops); for streaming newline-delimited
//! JSON from stdin see [`bulk_write`](bulk_write).
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
//! ```bash
//! cargo run --example bulk_write_single_op
//! ```

use udex_sdk::{
    bulk_write_entry_operation, bulk_write_entry_operation_result, BulkWriteEntryOperation,
    ClientOptions, ContextInput, CreateEntryRequest, KeyValuePair, UdexClient, Value,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv_override().ok();
    tracing_subscriber::fmt::init();

    let (client, index) = connect().await?;

    // A batch of three creates — all the same operation type. Each entry gets a
    // fresh server-generated key.
    let operations: Vec<BulkWriteEntryOperation> = [
        [("user_id", "1"), ("region", "eu")],
        [("user_id", "2"), ("region", "us")],
        [("user_id", "3"), ("region", "eu")],
    ]
    .into_iter()
    .map(|pairs| BulkWriteEntryOperation {
        operation: Some(bulk_write_entry_operation::Operation::CreateEntry(
            CreateEntryRequest {
                index_name: index.clone(),
                context: Some(context(&pairs)),
            },
        )),
    })
    .collect();

    let n = operations.len();
    let results = client.bulk_write(&index, operations).await?;

    println!("Wrote {n} entries in one transaction:");
    for (i, result) in results.iter().enumerate() {
        match result
            .result
            .as_ref()
            .ok_or_else(|| format!("result[{i}]: server returned empty result"))?
        {
            bulk_write_entry_operation_result::Result::CreateEntry(r) => {
                println!("  [{i}] created key={} hash={}", r.key, r.context_hash);
            }
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
