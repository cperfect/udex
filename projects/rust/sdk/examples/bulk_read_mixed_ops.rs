//! A **mixed-operation-type** `bulk_read`: resolve some entries key -> context
//! and others context -> key in a single batch.
//!
//! `bulk_read` accepts a heterogeneous batch — each operation independently
//! picks its direction via the `Operation` oneof:
//! - `LookupContext` takes an entry key and returns its context.
//! - `LookupKey` takes a `context_hash` and returns the entry key.
//!
//! Results come back in input order with a matching result variant per slot.
//!
//! Worth noting (shown below): `LookupKey` is addressed by a `context_hash` that
//! the **client** computes with [`xxh3_context_hash`] — a real reverse lookup
//! rarely has the server's hash to hand, so it hashes the context pairs itself.
//!
//! `bulk_read` needs entries to read, so this example first seeds a few with
//! `create_entry`, keeping both their keys and their contexts.
//!
//! For a single-op-type batch see [`bulk_read_single_op`](bulk_read_single_op).
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
//! cargo run --example bulk_read_mixed_ops
//! ```
//!
//! [`xxh3_context_hash`]: udex_sdk::xxh3_context_hash

use udex_api::entry::{
    bulk_read_entry_operation, bulk_read_entry_operation_result, LookupContextByKeyRequest,
    LookupKeyByContextRequest,
};
use udex_sdk::{
    xxh3_context_hash, BulkReadEntryOperation, ClientOptions, ContextInput, KeyValuePair,
    UdexClient, Value,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv_override().ok();
    tracing_subscriber::fmt::init();

    let (client, index) = connect().await?;

    // Seed a few entries, keeping both the key and the context we created it from
    // (the context is what a reverse LookupKey hashes over).
    let mut seeded: Vec<(String, ContextInput)> = Vec::new();
    for pairs in [
        [("user_id", "1"), ("region", "eu")],
        [("user_id", "2"), ("region", "us")],
        [("user_id", "3"), ("region", "eu")],
        [("user_id", "4"), ("region", "us")],
    ] {
        let ctx = context(&pairs);
        let resp = client.create_entry(&index, ctx.clone()).await?;
        seeded.push((resp.key, ctx));
    }

    // Mix directions: even slots resolve key -> context (LookupContext), odd slots
    // resolve context -> key (LookupKey) using a client-computed hash.
    let operations: Vec<BulkReadEntryOperation> = seeded
        .iter()
        .enumerate()
        .map(|(i, (key, ctx))| {
            let operation = if i % 2 == 0 {
                bulk_read_entry_operation::Operation::LookupContext(LookupContextByKeyRequest {
                    index_name: index.clone(),
                    key: key.clone(),
                })
            } else {
                let context_hash = xxh3_context_hash(ctx)?;
                bulk_read_entry_operation::Operation::LookupKey(LookupKeyByContextRequest {
                    index_name: index.clone(),
                    context_hash,
                })
            };
            Ok(BulkReadEntryOperation {
                operation: Some(operation),
            })
        })
        .collect::<Result<_, Box<dyn std::error::Error>>>()?;

    let results = client.bulk_read(&index, operations).await?;

    println!("Mixed read batch results (input order):");
    for (i, result) in results.iter().enumerate() {
        match result
            .result
            .as_ref()
            .ok_or_else(|| format!("result[{i}]: server returned empty result"))?
        {
            bulk_read_entry_operation_result::Result::LookupContext(r) => match &r.context {
                Some(ctx) => println!(
                    "  [{i}] key -> context: key={} hash={} ({} pairs)",
                    seeded[i].0,
                    ctx.hash,
                    ctx.pairs.len()
                ),
                None => println!("  [{i}] key -> context: key={} not found", seeded[i].0),
            },
            bulk_read_entry_operation_result::Result::LookupKey(r) => {
                println!(
                    "  [{i}] context -> key: {}",
                    r.key.as_deref().unwrap_or("<not found>")
                );
            }
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
