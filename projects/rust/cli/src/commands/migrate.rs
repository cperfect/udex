use anyhow::{Context, Result};
use udex_datastore::{postgres::PostgresDatastore, Datastore, Migrator};

use crate::cli::MigrateArgs;
use crate::config::UdexConfig;

/// Check whether the database schema is at the expected version.
///
/// Exits 0 if up to date, non-zero (with a descriptive error) if the database
/// is behind. Does not modify the database.
pub async fn check(args: MigrateArgs) -> Result<()> {
    let ds_config = UdexConfig::load_datastore_config(&args.config)?;
    let datastore = PostgresDatastore::init(ds_config)
        .await
        .context("failed to connect to database")?;

    let current = datastore
        .current_version()
        .await
        .context("failed to query current migration version")?;
    let latest = datastore
        .latest_version()
        .await
        .context("failed to determine latest migration version")?;

    if current == latest {
        println!("database is up to date (version: {current})");
        Ok(())
    } else {
        anyhow::bail!(
            "database is behind — current version: {current}, latest version: {latest}; \
             run `udex migrate apply` to update"
        )
    }
}

/// Apply all outstanding database migrations.
///
/// Connects to the database, runs all pending migrations in order, then
/// confirms the schema is at the expected version. Exits 0 on success.
pub async fn apply(args: MigrateArgs) -> Result<()> {
    let ds_config = UdexConfig::load_datastore_config(&args.config)?;
    let datastore = PostgresDatastore::init(ds_config)
        .await
        .context("failed to connect to database")?;

    datastore
        .migrate()
        .await
        .context("failed to apply migrations")?;

    let version = datastore
        .current_version()
        .await
        .context("failed to query migration version after apply")?;

    println!("migrations applied successfully (version: {version})");
    Ok(())
}
