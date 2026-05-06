use crate::config::DatastoreConfig;
use crate::{
    BulkOperationError, Datastore, Entry, EntryReadOperation, EntryReadResult, EntryWriteOperation,
    EntryWriteResult, Error, Migrator, UuidWrapper,
};
use serde_json;
use sqlx::postgres::PgPool;
use sqlx::Row;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use time::OffsetDateTime;
use udex_api::entry::{Context, KeyValuePair};
use udex_api::index::{HashAlgorithm, Index, IndexUpdate};
use udex_api::{google_timestamp_to_offset_datetime, offset_datetime_to_google_timestamp};
use uuid::Uuid;

pub struct PostgresDatastore {
    pool: Arc<PgPool>,
}

impl PostgresDatastore {
    fn new(_config: DatastoreConfig, pool: PgPool) -> Self {
        Self {
            pool: Arc::new(pool),
        }
    }
}

impl PostgresDatastore {
    /// Create an entry within an existing transaction.
    ///
    /// Inserts into `entry_context` with `ON CONFLICT (index_name, context_hash) DO NOTHING`.
    /// If the insert lands (rows_affected == 1) the provided key is returned.
    /// If the context already exists within the same index the existing key is fetched
    /// and returned, implementing the idempotent 1:1-per-index contract.
    async fn create_entry_tx(
        &self,
        entry: Entry,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> Result<Uuid, Error> {
        let hash_algorithm_str =
            sqlx::query_scalar::<_, String>("SELECT hash_algorithm FROM index WHERE name = $1")
                .bind(&entry.index_name)
                .fetch_optional(&mut **tx)
                .await
                .map_err(Error::Database)?
                .ok_or_else(|| {
                    Error::InvalidIndex(format!("Index '{}' not found", entry.index_name))
                })?;

        let result = sqlx::query(
            r#"
            INSERT INTO entry_context (
                key,
                index_name,
                context_hash,
                pairs,
                dek,
                kek_id,
                hash_algorithm
            )
            VALUES (
                $1,
                $2,
                $3,
                $4,
                $5,
                $6,
                $7
            )
            ON CONFLICT (index_name, context_hash) DO NOTHING
            "#,
        )
        .bind(UuidWrapper(entry.key))
        .bind(&entry.index_name)
        .bind(&entry.context.hash)
        .bind(serde_json::to_value(&entry.context.pairs).map_err(Error::Serialization)?)
        .bind(&entry.context.dek)
        .bind(&entry.context.kek_id)
        .bind(hash_algorithm_str)
        .execute(&mut **tx)
        .await
        .map_err(Error::Database)?;

        if result.rows_affected() == 1 {
            return Ok(entry.key);
        }

        // Context already existed within this index — fetch the pre-existing key.
        let existing_key: Uuid = sqlx::query_scalar(
            "SELECT key FROM entry_context WHERE index_name = $1 AND context_hash = $2",
        )
        .bind(&entry.index_name)
        .bind(&entry.context.hash)
        .fetch_one(&mut **tx)
        .await
        .map_err(Error::Database)?;

        Ok(existing_key)
    }

    /// Delete an entry within an existing transaction.
    async fn delete_entry_tx(
        &self,
        key: Uuid,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> Result<(), Error> {
        let result = sqlx::query(
            r#"
            DELETE FROM entry_context
            WHERE key = $1
            "#,
        )
        .bind(UuidWrapper(key))
        .execute(&mut **tx)
        .await
        .map_err(Error::Database)?;

        if result.rows_affected() == 0 {
            return Err(Error::EntryForKeyNotFound(key.to_string()));
        }

        Ok(())
    }

    /// Get an entry by key, using an arbitrary executor (pool or transaction).
    async fn get_entry_by_key_ex<'e, E>(&self, key: Uuid, executor: E) -> Result<Entry, Error>
    where
        E: sqlx::Executor<'e, Database = sqlx::Postgres>,
    {
        let row = sqlx::query(
            r#"
            SELECT
                key,
                index_name,
                context_hash,
                pairs,
                dek,
                kek_id,
                hash_algorithm
            FROM
                entry_context
            WHERE
                key = $1
            "#,
        )
        .bind(key)
        .fetch_one(executor)
        .await
        .map_err(Error::Database)?;

        row_to_entry(&row)
    }

    /// Get the single entry for a context hash within a named index, using an arbitrary executor.
    async fn get_entry_by_context_ex<'e, E>(
        &self,
        index_name: &str,
        context_hash: &str,
        executor: E,
    ) -> Result<Option<Entry>, Error>
    where
        E: sqlx::Executor<'e, Database = sqlx::Postgres>,
    {
        let row = sqlx::query(
            r#"
            SELECT
                key,
                index_name,
                context_hash,
                pairs,
                dek,
                kek_id,
                hash_algorithm
            FROM
                entry_context
            WHERE
                index_name = $1
                AND context_hash = $2
            "#,
        )
        .bind(index_name)
        .bind(context_hash)
        .fetch_optional(executor)
        .await
        .map_err(Error::Database)?;

        row.map(|r| row_to_entry(&r)).transpose()
    }
}

/// Map a database row from `entry_context` to an `Entry`.
fn row_to_entry(row: &sqlx::postgres::PgRow) -> Result<Entry, Error> {
    let pairs_value: serde_json::Value = row.try_get("pairs").map_err(Error::Database)?;
    let pairs: Vec<KeyValuePair> =
        serde_json::from_value(pairs_value).map_err(Error::Serialization)?;

    let context = Context {
        hash: row.try_get("context_hash").map_err(Error::Database)?,
        pairs,
        dek: row.try_get("dek").map_err(Error::Database)?,
        kek_id: row.try_get("kek_id").map_err(Error::Database)?,
    };

    Ok(Entry {
        key: row.try_get("key").map_err(Error::Database)?,
        context,
        index_name: row.try_get("index_name").map_err(Error::Database)?,
    })
}

#[async_trait::async_trait]
impl Datastore for PostgresDatastore {
    type Database = sqlx::Postgres;

    async fn init(config: DatastoreConfig) -> Result<Box<Self>, Error> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(config.max_connections)
            .min_connections(config.min_connections)
            .acquire_timeout(config.connection_timeout)
            .idle_timeout(config.query_timeout)
            .connect(
                &config
                    .resolved_connection_url()
                    .map_err(|e| Error::DatabaseNotInitialized(e.to_string()))?,
            )
            .await
            .map_err(Error::Database)?;

        sqlx::query("SELECT 1")
            .execute(&pool)
            .await
            .map_err(Error::Database)?;

        let row = sqlx::query("SHOW TRANSACTION ISOLATION LEVEL")
            .fetch_one(&pool)
            .await
            .map_err(Error::Database)?;

        let isolation_level: String = row.try_get(0).map_err(|e| {
            Error::DataConversion(format!("Failed to extract isolation level: {}", e))
        })?;

        match isolation_level.as_str() {
            "read committed" => (),
            _ => {
                return Err(Error::DatabaseNotInitialized(format!(
                    "Unsupported isolation level: {}",
                    isolation_level
                )))
            }
        }

        Ok(Box::new(PostgresDatastore::new(config, pool)))
    }

    async fn is_healthy(&self) -> Result<bool, Error> {
        sqlx::query("SELECT 1")
            .execute(&*self.pool)
            .await
            .map_err(Error::Database)?;
        Ok(true)
    }

    async fn create_index(&self, index: Index) -> Result<(), Error> {
        let created_at = {
            match google_timestamp_to_offset_datetime(index.created_at) {
                Err(_) => {
                    return Err(Error::InvalidIndex(
                        "Index created_at failed to convert".to_string(),
                    ))
                }
                Ok(None) => {
                    return Err(Error::InvalidIndex(
                        "Index must have a created_at timestamp".to_string(),
                    ))
                }
                Ok(Some(dt)) => dt,
            }
        };

        let hash_algorithm_str = HashAlgorithm::try_from(index.hash_algorithm)
            .map_err(|_| {
                Error::InvalidIndex(format!(
                    "Invalid hash_algorithm value: {}",
                    index.hash_algorithm
                ))
            })?
            .as_str_name();

        sqlx::query(
            r#"
            INSERT INTO index (
                name,
                description,
                max_bulk_operations,
                max_key_length,
                max_value_length,
                max_kv_pairs_per_context,
                hash_algorithm,
                created_at,
                created_by
            )
            VALUES (
                $1,
                $2,
                $3,
                $4,
                $5,
                $6,
                $7,
                $8,
                $9
            )
            "#,
        )
        .bind(&index.name)
        .bind(&index.description)
        .bind(index.max_bulk_operations)
        .bind(index.max_key_length)
        .bind(index.max_value_length)
        .bind(index.max_kv_pairs_per_context)
        .bind(hash_algorithm_str)
        .bind(created_at)
        .bind(&index.created_by)
        .execute(&*self.pool)
        .await
        .map_err(Error::Database)?;

        Ok(())
    }

    async fn get_index(&self, name: &str) -> Result<Index, Error> {
        let row = sqlx::query(
            r#"
            SELECT
                name,
                description,
                max_bulk_operations,
                max_key_length,
                max_value_length,
                max_kv_pairs_per_context,
                hash_algorithm,
                created_at,
                created_by,
                updated_at,
                updated_by
            FROM
                index
            WHERE
                name = $1
            "#,
        )
        .bind(name)
        .fetch_one(&*self.pool)
        .await
        .map_err(Error::Database)?;

        let created_at = offset_datetime_to_google_timestamp(
            row.try_get::<Option<OffsetDateTime>, _>("created_at")
                .map_err(Error::Database)?,
        )
        .map_err(|e| Error::DataConversion(format!("created_at conversion failed: {:?}", e)))?;

        let updated_at = offset_datetime_to_google_timestamp(
            row.try_get::<Option<OffsetDateTime>, _>("updated_at")
                .map_err(Error::Database)?,
        )
        .map_err(|e| Error::DataConversion(format!("updated_at conversion failed: {:?}", e)))?;

        let hash_algorithm_str: &str = row.try_get("hash_algorithm").map_err(Error::Database)?;
        let hash_algorithm = HashAlgorithm::from_str_name(hash_algorithm_str).ok_or_else(|| {
            Error::InvalidIndex(format!(
                "Invalid hash_algorithm value: {}",
                hash_algorithm_str
            ))
        })? as i32;

        Ok(Index {
            name: row.try_get("name").map_err(Error::Database)?,
            description: row.try_get("description").map_err(Error::Database)?,
            max_bulk_operations: row
                .try_get("max_bulk_operations")
                .map_err(Error::Database)?,
            max_key_length: row.try_get("max_key_length").map_err(Error::Database)?,
            max_value_length: row.try_get("max_value_length").map_err(Error::Database)?,
            max_kv_pairs_per_context: row
                .try_get("max_kv_pairs_per_context")
                .map_err(Error::Database)?,
            hash_algorithm,
            created_at,
            created_by: row.try_get("created_by").map_err(Error::Database)?,
            updated_at,
            updated_by: row.try_get("updated_by").map_err(Error::Database)?,
        })
    }

    async fn update_index(
        &self,
        name: &str,
        update: IndexUpdate,
        updated_by: &str,
    ) -> Result<Index, Error> {
        if update.description.is_none()
            && update.max_bulk_operations.is_none()
            && update.max_key_length.is_none()
            && update.max_value_length.is_none()
            && update.max_kv_pairs_per_context.is_none()
            && update.hash_algorithm.is_none()
        {
            return Err(Error::InvalidIndexUpdate("No fields to update".to_string()));
        }

        let now = OffsetDateTime::now_utc();

        sqlx::query(
            r#"
            UPDATE index
            SET
                description = COALESCE($1, description),
                max_bulk_operations = COALESCE($2, max_bulk_operations),
                max_key_length = COALESCE($3, max_key_length),
                max_value_length = COALESCE($4, max_value_length),
                max_kv_pairs_per_context = COALESCE($5, max_kv_pairs_per_context),
                hash_algorithm = COALESCE($6, hash_algorithm),
                updated_at = $7,
                updated_by = $8
            WHERE name = $9
            "#,
        )
        .bind(&update.description)
        .bind(update.max_bulk_operations)
        .bind(update.max_key_length)
        .bind(update.max_value_length)
        .bind(update.max_kv_pairs_per_context)
        .bind(match update.hash_algorithm {
            Some(h) => match HashAlgorithm::try_from(h) {
                Ok(alg) => Some(alg.as_str_name()),
                Err(_) => {
                    return Err(Error::InvalidIndex(format!(
                        "Invalid hash_algorithm value: {}",
                        h
                    )))
                }
            },
            None => None,
        })
        .bind(now)
        .bind(updated_by)
        .bind(name)
        .execute(&*self.pool)
        .await
        .map_err(Error::Database)?;

        self.get_index(name).await
    }

    async fn list_indices(&self) -> Result<Vec<Index>, Error> {
        let rows = sqlx::query(
            r#"
            SELECT
                name,
                description,
                max_bulk_operations,
                max_key_length,
                max_value_length,
                max_kv_pairs_per_context,
                hash_algorithm,
                created_at,
                created_by,
                updated_at,
                updated_by
            FROM
                index
            ORDER BY
                name
            "#,
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Error::Database)?;

        rows.into_iter()
            .map(|row| {
                let created_at = offset_datetime_to_google_timestamp(
                    row.try_get::<Option<OffsetDateTime>, _>("created_at")
                        .map_err(Error::Database)?,
                )
                .map_err(|e| {
                    Error::InvalidIndex(format!("created_at conversion failed: {:?}", e))
                })?;

                let updated_at = offset_datetime_to_google_timestamp(
                    row.try_get::<Option<OffsetDateTime>, _>("updated_at")
                        .map_err(Error::Database)?,
                )
                .map_err(|e| {
                    Error::InvalidIndex(format!("updated_at conversion failed: {:?}", e))
                })?;

                let hash_algorithm_str: &str =
                    row.try_get("hash_algorithm").map_err(Error::Database)?;
                let hash_algorithm =
                    HashAlgorithm::from_str_name(hash_algorithm_str).ok_or_else(|| {
                        Error::InvalidIndex(format!(
                            "Invalid hash_algorithm value: {}",
                            hash_algorithm_str
                        ))
                    })? as i32;

                Ok(Index {
                    name: row.try_get("name").map_err(Error::Database)?,
                    description: row.try_get("description").map_err(Error::Database)?,
                    max_bulk_operations: row
                        .try_get("max_bulk_operations")
                        .map_err(Error::Database)?,
                    max_key_length: row.try_get("max_key_length").map_err(Error::Database)?,
                    max_value_length: row.try_get("max_value_length").map_err(Error::Database)?,
                    max_kv_pairs_per_context: row
                        .try_get("max_kv_pairs_per_context")
                        .map_err(Error::Database)?,
                    hash_algorithm,
                    created_at,
                    created_by: row.try_get("created_by").map_err(Error::Database)?,
                    updated_at,
                    updated_by: row.try_get("updated_by").map_err(Error::Database)?,
                })
            })
            .collect::<Result<Vec<Index>, Error>>()
    }

    async fn create_entry(&self, entry: Entry) -> Result<Uuid, Error> {
        let mut tx = self.pool.begin().await.map_err(Error::Database)?;
        let key = self.create_entry_tx(entry, &mut tx).await?;
        tx.commit().await.map_err(Error::Database)?;
        Ok(key)
    }

    async fn get_entry_by_key(&self, key: Uuid) -> Result<Entry, Error> {
        self.get_entry_by_key_ex(key, &*self.pool).await
    }

    async fn get_entry_by_context(
        &self,
        index_name: &str,
        context_hash: &str,
    ) -> Result<Option<Entry>, Error> {
        self.get_entry_by_context_ex(index_name, context_hash, &*self.pool)
            .await
    }

    async fn delete_entry(&self, key: Uuid) -> Result<(), Error> {
        let mut tx = self.pool.begin().await.map_err(Error::Database)?;
        self.delete_entry_tx(key, &mut tx).await?;
        tx.commit().await.map_err(Error::Database)?;
        Ok(())
    }

    async fn bulk_entry_read(
        &self,
        operations: Vec<EntryReadOperation>,
    ) -> Result<Vec<EntryReadResult>, BulkOperationError> {
        let mut results = Vec::new();

        for operation in operations {
            let result: Result<EntryReadResult, Error> = match operation {
                EntryReadOperation::GetByKey(key) => self
                    .get_entry_by_key_ex(key, &*self.pool)
                    .await
                    .map(EntryReadResult::Entry),
                EntryReadOperation::GetByContext {
                    index_name,
                    context_hash,
                } => self
                    .get_entry_by_context_ex(&index_name, &context_hash, &*self.pool)
                    .await
                    .map(EntryReadResult::EntryByContext),
            };
            match result {
                Ok(res) => results.push(res),
                Err(e) => {
                    return Err(BulkOperationError {
                        msg: "Bulk read operation failed".to_string(),
                        source: e,
                    });
                }
            }
        }

        Ok(results)
    }

    async fn bulk_entry_write(
        &self,
        operations: Vec<EntryWriteOperation>,
    ) -> Result<Vec<EntryWriteResult>, BulkOperationError> {
        let mut tx = self.pool.begin().await.map_err(|e| BulkOperationError {
            msg: "Failed to begin transaction".to_string(),
            source: Error::Database(e),
        })?;

        let mut results = Vec::with_capacity(operations.len());

        for operation in operations {
            let result: Result<EntryWriteResult, Error> = match operation {
                EntryWriteOperation::Create(entry) => self
                    .create_entry_tx(entry, &mut tx)
                    .await
                    .map(EntryWriteResult::Created),
                EntryWriteOperation::Delete(key) => self
                    .delete_entry_tx(key, &mut tx)
                    .await
                    .map(|_| EntryWriteResult::Deleted),
            };
            match result {
                Ok(r) => results.push(r),
                Err(e) => {
                    return Err(BulkOperationError {
                        msg: "Bulk operation failed".to_string(),
                        source: e,
                    });
                }
            }
        }

        tx.commit().await.map_err(|e| BulkOperationError {
            msg: "Failed to commit transaction".to_string(),
            source: Error::Database(e),
        })?;

        Ok(results)
    }
}

#[async_trait::async_trait]
impl Migrator for PostgresDatastore {
    async fn migrate(&self) -> Result<(), Error> {
        tracing::info!("Running migrations");
        sqlx::migrate!("migrations/postgres")
            .run(&*self.pool)
            .await
            .map_err(|e| Error::Migration(format!("Migration failed: {}", e)))?;
        tracing::info!("Migrations completed successfully");
        Ok(())
    }

    async fn current_version(&self) -> Result<i64, Error> {
        let current_version: Option<i64> =
            sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations")
                .fetch_one(&*self.pool)
                .await
                .map_err(|err| {
                    Error::DatabaseNotInitialized(format!(
                        "Failed to check migration version: {}",
                        err
                    ))
                })?;

        current_version.ok_or_else(|| {
            Error::DatabaseNotInitialized("No migrations have been applied".to_string())
        })
    }

    async fn latest_version(&self) -> Result<i64, Error> {
        let migration_dir = Path::new("migrations/postgres");

        let migration_files = fs::read_dir(migration_dir).map_err(|err| {
            Error::DatabaseNotInitialized(format!("Failed to read migrations directory: {}", err))
        })?;

        let mut max_migration_version: Option<i64> = None;

        for entry in migration_files {
            let entry = entry.map_err(|err| {
                Error::DatabaseNotInitialized(format!("Failed to read migration file: {}", err))
            })?;
            let file_name = entry.file_name();
            let file_name_str = file_name.to_string_lossy();

            if let Some(name_without_ext) = file_name_str.strip_suffix(".sql") {
                if let Some(underscore_pos) = name_without_ext.find('_') {
                    let version_str = &name_without_ext[..underscore_pos];
                    let name_part = &name_without_ext[underscore_pos + 1..];

                    if version_str.chars().all(|c| c.is_ascii_digit())
                        && name_part
                            .chars()
                            .all(|c| c.is_ascii_lowercase() || c == '_')
                    {
                        if let Ok(version) = version_str.parse::<i64>() {
                            max_migration_version =
                                Some(max_migration_version.unwrap_or(0).max(version));
                        }
                    }
                }
            }
        }

        max_migration_version
            .ok_or_else(|| Error::DatabaseNotInitialized("No migrations found".to_string()))
    }
}
