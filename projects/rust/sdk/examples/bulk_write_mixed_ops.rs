//! A **mixed-operation-type** `bulk_write`: create, lookup-or-create, and delete
//! in a single transaction.
//!
//! `bulk_write` accepts a heterogeneous batch — each operation independently
//! chooses its variant via the `Operation` oneof. All operations still share one
//! index and commit atomically, and results come back in input order with a
//! matching result variant per slot.
//!
//! Two things worth noting, both shown below:
//! - `LookupOrCreate` carries a `context_hash` that the **client** computes with
//!   [`xxh3_context_hash`] (the server recomputes and rejects a mismatch). The
//!   `lookup_or_create_entry` convenience method hides this; a hand-built bulk op
//!   does not, so you compute it yourself.
//! - `DeleteEntry` needs a key, so this example first creates a throwaway "seed"
//!   entry to obtain one, then deletes it inside the mixed batch.
//!
//! For a single-op-type batch see [`bulk_write_single_op`](bulk_write_single_op).
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
//! cargo run --example bulk_write_mixed_ops
//! ```
//!
//! [`xxh3_context_hash`]: udex_sdk::xxh3_context_hash

use udex_sdk::{
    bulk_write_entry_operation, bulk_write_entry_operation_result, xxh3_context_hash,
    BulkWriteEntryOperation, ClientOptions, ContextInput, CreateEntryRequest, DeleteEntryRequest,
    KeyValuePair, LookupKeyByContextOrCreateRequest, UdexClient, Value,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv_override().ok();
    tracing_subscriber::fmt::init();

    let (client, index) = connect().await?;

    // Seed an entry up front so the mixed batch below has a real key to delete.
    let seed = client
        .create_entry(&index, context(&[("seed", "delete-me")]))
        .await?;
    println!("Seeded entry to delete: key={}", seed.key);

    // A single batch mixing three operation types:
    //   0: CreateEntry     — always makes a new entry
    //   1: LookupOrCreate   — returns the existing key or creates one (client hash)
    //   2: DeleteEntry      — removes the seed entry created above
    let loc_context = context(&[("user_id", "42"), ("tier", "gold")]);
    let loc_hash = xxh3_context_hash(&loc_context)?;

    let operations = vec![
        BulkWriteEntryOperation {
            operation: Some(bulk_write_entry_operation::Operation::CreateEntry(
                CreateEntryRequest {
                    index_name: index.clone(),
                    context: Some(context(&[("user_id", "7"), ("region", "eu")])),
                },
            )),
        },
        BulkWriteEntryOperation {
            operation: Some(bulk_write_entry_operation::Operation::LookupOrCreate(
                LookupKeyByContextOrCreateRequest {
                    index_name: index.clone(),
                    context: Some(loc_context),
                    context_hash: loc_hash,
                },
            )),
        },
        BulkWriteEntryOperation {
            operation: Some(bulk_write_entry_operation::Operation::DeleteEntry(
                DeleteEntryRequest {
                    index_name: index.clone(),
                    key: seed.key.clone(),
                },
            )),
        },
    ];

    let results = client.bulk_write(&index, operations).await?;

    println!("Mixed batch results (input order):");
    for (i, result) in results.iter().enumerate() {
        match result
            .result
            .as_ref()
            .ok_or_else(|| format!("result[{i}]: server returned empty result"))?
        {
            bulk_write_entry_operation_result::Result::CreateEntry(r) => {
                println!(
                    "  [{i}] create         key={} hash={}",
                    r.key, r.context_hash
                );
            }
            bulk_write_entry_operation_result::Result::LookupOrCreate(r) => {
                println!(
                    "  [{i}] lookup_or_create key={} hash={} created={}",
                    r.key, r.context_hash, r.created
                );
            }
            bulk_write_entry_operation_result::Result::DeleteEntry(_) => {
                println!("  [{i}] delete         ok (key={})", seed.key);
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
