//! Handlers for `udex index` subcommands.

use anyhow::{Context, Result};
use tabled::{Table, Tabled};
use udex_api::index::HashAlgorithm;
use udex_sdk::{CreateIndexRequest, Index, IndexUpdate, UdexClient};

use crate::cli::{IndexCreateArgs, IndexDeleteArgs, IndexGetArgs, IndexUpdateArgs, OutputFormat};

// ---------------------------------------------------------------------------
// Table display
// ---------------------------------------------------------------------------

#[derive(Tabled)]
struct IndexRow {
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Description")]
    description: String,
    #[tabled(rename = "Bulk Limit")]
    max_bulk_operations: i32,
    #[tabled(rename = "Max Key Len")]
    max_key_length: i32,
    #[tabled(rename = "Max Val Len")]
    max_value_length: i32,
    #[tabled(rename = "Max Context Pairs")]
    max_kv_pairs_per_context: i32,
}

impl From<Index> for IndexRow {
    fn from(i: Index) -> Self {
        IndexRow {
            name: i.name,
            description: i.description,
            max_bulk_operations: i.max_bulk_operations,
            max_key_length: i.max_key_length,
            max_value_length: i.max_value_length,
            max_kv_pairs_per_context: i.max_kv_pairs_per_context,
        }
    }
}

fn print_index(index: Index, output: &OutputFormat) -> Result<()> {
    match output {
        OutputFormat::Table => {
            println!("{}", Table::new([IndexRow::from(index)]));
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&index)?);
        }
        OutputFormat::Yaml => {
            print!("{}", serde_yaml::to_string(&index)?);
        }
    }
    Ok(())
}

fn print_indices(indices: Vec<Index>, output: &OutputFormat) -> Result<()> {
    match output {
        OutputFormat::Table => {
            let rows: Vec<IndexRow> = indices.into_iter().map(IndexRow::from).collect();
            println!("{}", Table::new(rows));
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&indices)?);
        }
        OutputFormat::Yaml => {
            print!("{}", serde_yaml::to_string(&indices)?);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Command handlers
// ---------------------------------------------------------------------------

/// List all indices.
pub async fn list(client: &UdexClient, output: &OutputFormat) -> Result<()> {
    let indices = client
        .list_indices()
        .await
        .context("list_indices RPC failed")?;
    print_indices(indices, output)
}

/// Create a new index.
pub async fn create(
    client: &UdexClient,
    args: IndexCreateArgs,
    output: &OutputFormat,
) -> Result<()> {
    use std::convert::TryFrom;
    let max_bulk_operations = i32::try_from(args.bulk_limit)
        .map_err(|_| anyhow::anyhow!("bulk_limit too large (must fit in i32)"))?;
    let max_key_length = i32::try_from(args.max_key_length)
        .map_err(|_| anyhow::anyhow!("max_key_length too large (must fit in i32)"))?;
    let max_value_length = i32::try_from(args.max_value_length)
        .map_err(|_| anyhow::anyhow!("max_value_length too large (must fit in i32)"))?;
    let max_kv_pairs_per_context = i32::try_from(args.max_context_pairs)
        .map_err(|_| anyhow::anyhow!("max_context_pairs too large (must fit in i32)"))?;

    let index = client
        .create_index(CreateIndexRequest {
            name: args.name,
            description: args.description.unwrap_or_default(),
            max_bulk_operations,
            max_key_length,
            max_value_length,
            max_kv_pairs_per_context,
            hash_algorithm: HashAlgorithm::Xxh3 as i32,
        })
        .await
        .context("create_index RPC failed")?;

    print_index(index, output)
}

/// Get an index by name.
pub async fn get(client: &UdexClient, args: IndexGetArgs, output: &OutputFormat) -> Result<()> {
    let index = client
        .describe_index(&args.name)
        .await
        .context("describe RPC failed")?;
    print_index(index, output)
}

/// Update an existing index.
pub async fn update(
    client: &UdexClient,
    args: IndexUpdateArgs,
    output: &OutputFormat,
) -> Result<()> {
    let max_bulk_operations = args
        .bulk_limit
        .map(i32::try_from)
        .transpose()
        .map_err(|_| anyhow::anyhow!("bulk_limit too large (must fit in i32)"))?;

    let index = client
        .update_index(
            &args.name,
            IndexUpdate {
                description: args.description,
                max_bulk_operations,
                ..Default::default()
            },
        )
        .await
        .context("update_index RPC failed")?;

    print_index(index, output)
}

/// Delete an index.
pub async fn delete(
    client: &UdexClient,
    args: IndexDeleteArgs,
    _output: &OutputFormat,
) -> Result<()> {
    client.delete_index(&args.name).await.map_err(|e| {
        let msg = match &e {
            udex_sdk::Error::Rpc(s) if s.code() == udex_sdk::grpc_code::FAILED_PRECONDITION => {
                format!(
                    "index '{}' still has entries — delete all entries first",
                    args.name
                )
            }
            udex_sdk::Error::Rpc(s) if s.code() == udex_sdk::grpc_code::NOT_FOUND => {
                format!("index '{}' not found", args.name)
            }
            _ => "delete_index RPC failed".to_string(),
        };
        anyhow::Error::from(e).context(msg)
    })?;
    println!("Deleted index '{}'.", args.name);
    Ok(())
}
