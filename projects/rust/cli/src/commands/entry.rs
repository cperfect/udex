//! Handlers for `udex entry` subcommands.

use anyhow::{Context, Result};
use tabled::{Table, Tabled};
use udex_api::hash::xxh3_context_hash;
use udex_sdk::{value, ContextInput, KeyValuePair, UdexClient, Value};

use crate::cli::{
    EntryCreateArgs, EntryDeleteArgs, EntryGetArgs, EntryLookupArgs, EntryLookupOrCreateArgs,
    OutputFormat,
};

// ---------------------------------------------------------------------------
// Context K=V parsing
// ---------------------------------------------------------------------------

/// Parse `KEY=VALUE` strings into `KeyValuePair`s. Values are treated as strings.
fn parse_context_pairs(args: &[String]) -> Result<Vec<KeyValuePair>> {
    args.iter()
        .map(|s| {
            let (k, v) = s
                .split_once('=')
                .with_context(|| format!("context item must be KEY=VALUE, got: {s:?}"))?;
            Ok(KeyValuePair {
                key: k.to_string(),
                value: Some(Value {
                    value: Some(value::Value::StringValue(v.to_string())),
                }),
                kek_id: None,
                dek: None,
            })
        })
        .collect()
}

fn build_context_input(args: &[String]) -> Result<ContextInput> {
    Ok(ContextInput {
        pairs: parse_context_pairs(args)?,
    })
}

// ---------------------------------------------------------------------------
// Table display helpers
// ---------------------------------------------------------------------------

#[derive(Tabled)]
struct KvRow {
    #[tabled(rename = "Key")]
    key: String,
    #[tabled(rename = "Value")]
    value: String,
}

#[derive(Tabled)]
struct EntryKeyRow {
    #[tabled(rename = "Key")]
    key: String,
}

fn value_to_string(v: Option<Value>) -> String {
    match v.and_then(|v| v.value) {
        Some(value::Value::StringValue(s)) => s,
        Some(value::Value::IntValue(i)) => i.to_string(),
        Some(value::Value::FloatValue(f)) => f.to_string(),
        Some(value::Value::BoolValue(b)) => b.to_string(),
        None => String::new(),
    }
}

// ---------------------------------------------------------------------------
// Command handlers
// ---------------------------------------------------------------------------

/// Create a new entry.
pub async fn create(
    client: &UdexClient,
    args: EntryCreateArgs,
    output: &OutputFormat,
) -> Result<()> {
    let context = build_context_input(&args.context)?;
    let resp = client
        .create_entry(&args.index, context)
        .await
        .context("create_entry RPC failed")?;

    match output {
        OutputFormat::Table => {
            let rows = vec![
                KvRow {
                    key: "key".to_string(),
                    value: resp.key,
                },
                KvRow {
                    key: "context_hash".to_string(),
                    value: resp.context_hash,
                },
            ];
            println!("{}", Table::new(rows));
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
        OutputFormat::Yaml => {
            print!("{}", serde_saphyr::to_string(&resp)?);
        }
    }
    Ok(())
}

/// Get an entry's context by key.
pub async fn get(client: &UdexClient, args: EntryGetArgs, output: &OutputFormat) -> Result<()> {
    let ctx = client
        .lookup_context_by_key(&args.index, &args.key)
        .await
        .context("lookup_context_by_key RPC failed")?;

    match output {
        OutputFormat::Table => {
            let rows: Vec<KvRow> = ctx
                .pairs
                .into_iter()
                .map(|p| KvRow {
                    key: p.key,
                    value: value_to_string(p.value),
                })
                .collect();
            println!("{}", Table::new(rows));
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&ctx)?);
        }
        OutputFormat::Yaml => {
            print!("{}", serde_saphyr::to_string(&ctx)?);
        }
    }
    Ok(())
}

/// Reverse-lookup the entry key for a given context hash. Prints the key when found,
/// or a "not found" message when no entry exists for the context.
pub async fn lookup(
    client: &UdexClient,
    args: EntryLookupArgs,
    output: &OutputFormat,
) -> Result<()> {
    let context = build_context_input(&args.context)?;
    let context_hash = xxh3_context_hash(&context).context("failed to compute context hash")?;

    match client
        .lookup_key_by_context(&args.index, &context_hash)
        .await
        .context("lookup_key_by_context RPC failed")?
    {
        None => match output {
            OutputFormat::Table => eprintln!("not found"),
            OutputFormat::Json => println!("null"),
            OutputFormat::Yaml => println!("~"),
        },
        Some(key) => match output {
            OutputFormat::Table => {
                println!("{}", Table::new(vec![EntryKeyRow { key }]));
            }
            OutputFormat::Json => {
                println!("{}", serde_json::to_string_pretty(&key)?);
            }
            OutputFormat::Yaml => {
                print!("{}", serde_saphyr::to_string(&key)?);
            }
        },
    }
    Ok(())
}

/// Look up the key for a context, creating the entry if it does not exist.
pub async fn lookup_or_create(
    client: &UdexClient,
    args: EntryLookupOrCreateArgs,
    output: &OutputFormat,
) -> Result<()> {
    let context = build_context_input(&args.context)?;
    let resp = client
        .lookup_or_create_entry(&args.index, context)
        .await
        .context("lookup_or_create_entry RPC failed")?;

    match output {
        OutputFormat::Table => {
            let rows = vec![
                KvRow {
                    key: "key".to_string(),
                    value: resp.key,
                },
                KvRow {
                    key: "context_hash".to_string(),
                    value: resp.context_hash,
                },
                KvRow {
                    key: "created".to_string(),
                    value: resp.created.to_string(),
                },
            ];
            println!("{}", Table::new(rows));
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
        OutputFormat::Yaml => {
            print!("{}", serde_saphyr::to_string(&resp)?);
        }
    }
    Ok(())
}

/// Delete an entry by key.
pub async fn delete(
    client: &UdexClient,
    args: EntryDeleteArgs,
    output: &OutputFormat,
) -> Result<()> {
    client
        .delete_entry(&args.index, &args.key)
        .await
        .context("delete_entry RPC failed")?;
    match output {
        OutputFormat::Table => println!("deleted"),
        OutputFormat::Json => println!("null"),
        OutputFormat::Yaml => println!("~"),
    }
    Ok(())
}
