use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgArgumentBuffer, Database, Encode, Type};
use thiserror::Error;
use udex_api::entry::Context;
use udex_api::index::{Index, IndexUpdate};
use uuid::Uuid;

use crate::config::DatastoreConfig;

pub mod config;
pub mod postgres;

#[cfg(feature = "integration_test")]
pub mod integration_test;

/// Trait for SQL type conversion
pub trait SqlType: Send + Sync {
    type Database: Database;
    fn type_info() -> <Self::Database as Database>::TypeInfo;
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Context not found: {0}")]
    EntryForKeyNotFound(String),
    #[error("Key not found: {0}")]
    EntryForContextNotFound(Uuid),
    #[error("Invalid context: {0}")]
    InvalidContext(String),
    #[error("Transaction error: {0}")]
    Transaction(String),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Invalid index: {0}")]
    InvalidIndex(String),
    #[error("Data conversion error: {0}")]
    DataConversion(String),
    /// Type of error that occurs when database is not properly initialized
    #[error("Database not initialized: {0}")]
    DatabaseNotInitialized(String),
    /// Type of error that occurs when a requested operation is not implemented
    #[error("Not implemented: {0}")]
    NotImplemented(String),
    #[error("Invalid index update: {0}")]
    InvalidIndexUpdate(String),
    #[error("Index not empty: {0}")]
    IndexNotEmpty(String),
    // Type of error related to migrations
    #[error("Migration error: {0}")]
    Migration(String),
}

/// Type of error that occurs when a bulk operation fails
/// derived as a separate type so it can reference the Error enum without causing a circular dependency
#[derive(Debug, Error)]
#[error("Bulk operation failed: {msg}")]
pub struct BulkOperationError {
    msg: String,
    #[source]
    source: crate::Error,
}

impl BulkOperationError {
    /// Returns the underlying datastore error, allowing callers to map it to
    /// a precise status code rather than collapsing everything to `internal`.
    pub fn into_source(self) -> crate::Error {
        self.source
    }
}

/// Wrapper type for Uuid to implement SQL traits
#[derive(Debug, Clone, Copy)]
pub struct UuidWrapper(pub Uuid);

impl SqlType for UuidWrapper {
    type Database = sqlx::Postgres;
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("uuid")
    }
}

impl<'q> Encode<'q, sqlx::Postgres> for UuidWrapper {
    fn encode_by_ref(
        &self,
        buf: &mut PgArgumentBuffer,
    ) -> std::result::Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync + 'static>>
    {
        buf.extend_from_slice(self.0.as_bytes());
        Ok(sqlx::encode::IsNull::No)
    }
}

impl Type<sqlx::Postgres> for UuidWrapper {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("uuid")
    }
}

/// Represents an Entry in the index: a mapping between a key and a context
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub key: Uuid,
    pub context: Context,
    pub index_name: String,
}

pub enum EntryWriteOperation {
    Create(Entry),
    Delete { index_name: String, key: Uuid },
}

/// Result of a single write operation in a bulk write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryWriteResult {
    /// The key that was stored — either newly created or the pre-existing key
    /// for this context (idempotent create).
    Created(Uuid),
    Deleted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryReadOperation {
    GetByKey(Uuid),
    GetByContext {
        index_name: String,
        context_hash: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum EntryReadResult {
    /// Result of a `GetByKey` operation.
    ByKey(Entry),
    /// Result of a `GetByContext` operation — `None` when no entry exists for the hash.
    EntryByContext(Option<Entry>),
}

/// Trait for datastore operations
#[async_trait::async_trait]
pub trait Datastore: Send + Sync {
    /// Initialize a datastore with the config and return a new instance.
    async fn init(config: DatastoreConfig) -> Result<Box<Self>, Error>
    where
        Self: Sized;

    /// Checks the server is healthy
    async fn is_healthy(&self) -> Result<bool, Error>;

    /// Create a new index
    async fn create_index(&self, index: Index) -> Result<(), Error>;

    /// Get an index by name
    async fn get_index(&self, name: &str) -> Result<Index, Error>;

    /// Update an existing index (only mutable fields)
    /// i.e. all but the name
    async fn update_index(
        &self,
        name: &str,
        update: IndexUpdate,
        updated_by: &str,
    ) -> Result<Index, Error>;

    /// List all indices
    async fn list_indices(&self) -> Result<Vec<Index>, Error>;

    /// Create a new entry. Idempotent on duplicate context: if an entry already
    /// exists for `entry.context.hash` within the index, the existing key is
    /// returned and no new row is written. The `entry.key` is used as the
    /// candidate key for new inserts; it is ignored when the context already exists.
    async fn create_entry(&self, entry: Entry) -> Result<Uuid, Error>;

    /// Look up the key for a context, creating the entry if it does not exist.
    ///
    /// Returns `(key, created)` where `created` is `true` when the entry was
    /// inserted by this call and `false` when a pre-existing entry was found.
    async fn lookup_or_create_entry(&self, entry: Entry) -> Result<(Uuid, bool), Error>;

    /// Get an entry by key
    async fn get_entry_by_key(&self, key: Uuid) -> Result<Entry, Error>;

    /// Get the single entry for a context hash within a named index, if one exists.
    async fn get_entry_by_context(
        &self,
        index_name: &str,
        context_hash: &str,
    ) -> Result<Option<Entry>, Error>;

    /// Delete an entry by key.
    async fn delete_entry(&self, index_name: &str, key: Uuid) -> Result<(), Error>;

    /// Delete an index by name. Returns `Error::IndexNotEmpty` if the index
    /// still has entries, and `Error::InvalidIndex` if it does not exist.
    async fn delete_index(&self, name: &str) -> Result<(), Error>;

    /// Perform multiple entry write operations in a single transaction.
    /// Any failure will rollback all operations.
    /// Returns one `EntryWriteResult` per operation in input order.
    async fn bulk_entry_write(
        &self,
        operations: Vec<EntryWriteOperation>,
    ) -> Result<Vec<EntryWriteResult>, BulkOperationError>;

    /// Perform multiple entry read operations in a single transaction.
    /// All reads will be executed in a single transaction and will have the same isolation level.
    async fn bulk_entry_read(
        &self,
        operations: Vec<EntryReadOperation>,
    ) -> Result<Vec<EntryReadResult>, BulkOperationError>;

    /// The database type for this datastore
    type Database: Database;
}

/// Trait for datastore migrations
#[async_trait::async_trait]
pub trait Migrator: Send + Sync {
    /// Migrate the datastore to the latest version.
    async fn migrate(&self) -> Result<(), Error>;

    /// Get the current version of the datastore.
    async fn current_version(&self) -> Result<i64, Error>;

    /// Get the latest version of the datastore.
    async fn latest_version(&self) -> Result<i64, Error>;

    /// check the migration version matches available migrations
    async fn check_migration_version(&self) -> Result<(), Error> {
        let current_version = self.current_version().await?;
        let latest_version = self.latest_version().await?;
        if current_version != latest_version {
            return Err(Error::DatabaseNotInitialized(format!(
                "Current version {} does not match latest version {}",
                current_version, latest_version
            )));
        }
        Ok(())
    }
}
