//! Batch-create entries in a Udex index using `bulk_write`.
//!
//! Reads newline-delimited JSON objects from stdin, where each object
//! contains the key-value pairs for one entry context.  Sends them all in
//! a single `bulk_write` RPC and prints the resulting entry keys.
//!
//! # Required environment variables
//!
//! | Variable             | Description |
//! |----------------------|-------------|
//! | `UDEX_ENDPOINT`      | gRPC server URL |
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
//! # Example
//!
//! ```bash
//! printf '{"user_id":"1","region":"eu"}\n{"user_id":"2","region":"us"}\n' |
//!     cargo run --example bulk_write
//! ```
//!
//! Each line must be a flat JSON object. String, number, and boolean values
//! are mapped to the corresponding SDK [`Value`] type; objects, arrays, and
//! null are rejected.
//!
//! [`Value`]: udex_sdk::Value

use std::io::{self, BufRead};

use udex_api::entry::{bulk_write_entry_operation, bulk_write_entry_operation_result};
use udex_sdk::{context_input_from_json, BulkWriteEntryOperation, ClientOptions, UdexClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv_override().ok();
    tracing_subscriber::fmt::init();

    let (client, index) = connect().await?;

    // Read newline-delimited JSON objects from stdin.
    let stdin = io::stdin();
    let mut operations: Vec<BulkWriteEntryOperation> = Vec::new();

    for line in stdin.lock().lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let obj: serde_json::Map<String, serde_json::Value> = serde_json::from_str(line)?;
        let context = context_input_from_json(obj)?;

        operations.push(BulkWriteEntryOperation {
            operation: Some(bulk_write_entry_operation::Operation::CreateEntry(
                udex_sdk::CreateEntryRequest {
                    index_name: index.clone(),
                    context: Some(context),
                },
            )),
        });
    }

    if operations.is_empty() {
        eprintln!("No input — pipe newline-delimited JSON objects to stdin.");
        return Ok(());
    }

    let n = operations.len();
    let results = client.bulk_write(&index, operations).await?;

    println!("Written {n} entries:");
    for (i, result) in results.iter().enumerate() {
        let inner = result
            .result
            .as_ref()
            .ok_or_else(|| format!("result[{i}]: server returned empty result"))?;
        match inner {
            bulk_write_entry_operation_result::Result::CreateEntry(r) => {
                println!("  [{i}] key={} hash={}", r.key, r.context_hash);
            }
            bulk_write_entry_operation_result::Result::DeleteEntry(_) => {
                println!("  [{i}] deleted");
            }
            bulk_write_entry_operation_result::Result::LookupOrCreate(r) => {
                println!(
                    "  [{i}] key={} hash={} created={}",
                    r.key, r.context_hash, r.created
                );
            }
        }
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
