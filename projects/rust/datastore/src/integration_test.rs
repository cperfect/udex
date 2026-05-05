use crate::{config::DatastoreConfig, postgres::PostgresDatastore, Datastore, Migrator};
/// This file contains helpers for integration tests
use maybe_once::tokio::{Data, MaybeOnceAsync};
use secrets_rs::{Secret, SourceError, SourceRegistry};
use sqlx::postgres::PgPoolOptions;
use std::sync::{Arc, Mutex, OnceLock};
use uuid::Uuid;

/// Single-value source used in test fixtures to bind a known URL without env vars.
struct StaticSource(String);
impl secrets_rs::Source for StaticSource {
    fn get(&self, _name: &str) -> Result<Vec<u8>, SourceError> {
        Ok(self.0.as_bytes().to_vec())
    }
}

fn bound_url_secret(url: &str) -> Secret<String> {
    let mut secret = Secret::new("urn:secrets-rs:test:url").unwrap();
    let mut registry = SourceRegistry::new();
    registry
        .register("test", StaticSource(url.to_string()))
        .unwrap();
    secret.bind(&registry).unwrap();
    secret
}

// See https://github.com/ufoscout/maybe-once for the MaybeOnceAsync pattern used here.
pub type MaybeOnceType = (
    // useful stuff for the tests
    PostgresDatastore,
    Arc<sqlx::PgPool>,
    // Database name for inspection/manual cleanup
    String,
);

/// Static storage for database name to enable cleanup
static TEST_DB_NAME: OnceLock<Mutex<Option<String>>> = OnceLock::new();

/// Register cleanup handler using ctor crate
/// This runs when the test binary exits
#[ctor::dtor]
fn cleanup_on_exit() {
    // Check if we should keep fixtures
    let keep_fixtures = std::env::var("KEEP_FIXTURES")
        .map(|v| v.to_lowercase() == "true" || v == "1")
        .unwrap_or(false);

    if keep_fixtures {
        if let Some(mutex) = TEST_DB_NAME.get() {
            if let Ok(guard) = mutex.lock() {
                if let Some(ref db_name) = *guard {
                    println!("KEEP_FIXTURES=true: Preserving test database '{}'", db_name);
                }
            }
        }
        return;
    }

    // Perform cleanup
    if let Some(mutex) = TEST_DB_NAME.get() {
        if let Ok(guard) = mutex.lock() {
            if let Some(ref db_name) = *guard {
                println!("Cleaning up test database on exit: {}", db_name);

                let db_name = db_name.clone();
                // Spawn a fresh thread to create the Tokio runtime.
                // Benchmarks call block_on() on the main thread, which marks Tokio's
                // CONTEXT thread-local as Destroyed when it returns. A fresh thread
                // has a clean CONTEXT, so Runtime::new() succeeds there.
                let handle = std::thread::spawn(move || {
                    let rt = tokio::runtime::Runtime::new()
                        .expect("test cleanup: failed to create Tokio runtime");
                    rt.block_on(async {
                        cleanup_test_database_internal(&db_name)
                            .await
                            .unwrap_or_else(|e| {
                                panic!("test cleanup: failed to drop database '{}': {}", db_name, e)
                            });
                        println!("Test database '{}' cleaned up successfully", db_name);
                    });
                });
                handle.join().unwrap_or_else(|e| {
                    panic!(
                        "test cleanup thread panicked: {}",
                        e.downcast_ref::<&str>()
                            .copied()
                            .or_else(|| e.downcast_ref::<String>().map(String::as_str))
                            .unwrap_or("unknown panic payload")
                    )
                });
            }
        }
    }
}

/// Initializer function.
/// Creates a new database in an existing PostgreSQL instance for testing.
/// The database will have a unique name and will be isolated from other test runs.
pub async fn init_postgres() -> MaybeOnceType {
    // Get the base database URL from environment variable
    let base_database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL environment variable must be set for integration tests");

    // Generate a unique database name for this test run
    let test_run_id = Uuid::new_v4().to_string().replace('-', "_");
    let test_db_name = format!("udex_datastore_integration_test_{}", test_run_id);

    println!("Creating test database: {}", test_db_name);

    // Connect to the base database (typically 'postgres') to create the test database
    let admin_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&base_database_url)
        .await
        .expect("Failed to connect to base database");

    // Create the test database
    // Note: We can't use parameterized queries for CREATE DATABASE
    let create_db_query = format!("CREATE DATABASE {}", test_db_name);
    sqlx::query(&create_db_query)
        .execute(&admin_pool)
        .await
        .expect("Failed to create test database");

    println!("Test database '{}' created successfully", test_db_name);

    // Close the admin connection
    admin_pool.close().await;

    // Parse the base URL to construct the test database URL
    let test_database_url = replace_database_name(&base_database_url, &test_db_name);

    println!("Connecting to test database: {}", test_database_url);

    // Initialize the datastore with the test database
    // Configure for concurrent test execution:
    // - 13 tests run concurrently
    // - test_concurrent_operations spawns 10 additional tasks
    // - Extra buffer for overhead
    let cfg = DatastoreConfig {
        connection_url: bound_url_secret(&test_database_url),
        max_connections: 25, // Increased to support concurrent tests
        min_connections: 1,
        connection_timeout: std::time::Duration::from_secs(30), // Long timeout for concurrent tests
        query_timeout: std::time::Duration::from_secs(30),
        dangerous_allow_non_tls: true, // local test DB has no TLS
    };

    let datastore = PostgresDatastore::init(cfg)
        .await
        .expect("Datastore initialization failed");

    // Get the pool from the datastore for direct SQL access in tests
    // Note: We create a separate connection for this to avoid affecting the datastore's pool
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&test_database_url)
        .await
        .expect("Failed to connect to test database");

    let pool_arc = Arc::new(pool);

    // Run migrations on the test database
    println!("Running migrations on test database...");
    datastore
        .migrate()
        .await
        .expect("Datastore migration failed");
    println!("Migrations completed successfully");

    // Register database name for cleanup on exit
    TEST_DB_NAME
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap()
        .replace(test_db_name.clone());

    (*datastore, pool_arc, test_db_name)
}

/// Helper function to replace the database name in a PostgreSQL connection URL
fn replace_database_name(url: &str, new_db_name: &str) -> String {
    // Parse the URL to replace the database name
    // Format: postgres://user:password@host:port/database?params
    if let Some(at_index) = url.rfind('@') {
        if let Some(slash_index) = url[at_index..].find('/') {
            let base_url = &url[..at_index + slash_index + 1];

            // Handle query parameters if present
            let rest = &url[at_index + slash_index + 1..];
            if let Some(query_index) = rest.find('?') {
                let params = &rest[query_index..];
                format!("{}{}{}", base_url, new_db_name, params)
            } else {
                format!("{}{}", base_url, new_db_name)
            }
        } else {
            // No database specified in URL, append it
            format!("{}/{}", url, new_db_name)
        }
    } else {
        panic!("Invalid PostgreSQL connection URL format");
    }
}

/// A function that holds a static reference to the test database
/// The database is created once per test run and shared across all tests
pub async fn data(serial: bool) -> Data<'static, MaybeOnceType> {
    static DATA: OnceLock<MaybeOnceAsync<MaybeOnceType>> = OnceLock::new();
    DATA.get_or_init(|| MaybeOnceAsync::new(|| Box::pin(init_postgres())))
        .data(serial)
        .await
}

/// Internal cleanup function that returns a Result
async fn cleanup_test_database_internal(db_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("Cleaning up test database: {}", db_name);

    // Read DATABASE_URL directly from the environment here — this is admin-pool
    // cleanup code, not application code, so bypassing Secret<T> is intentional.
    // The test fixture uses bound_url_secret() for the datastore under test.
    let base_database_url = std::env::var("DATABASE_URL")?;

    // Connect to the base database to drop the test database
    let admin_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&base_database_url)
        .await?;

    // Terminate existing connections to the test database
    let terminate_query = format!(
        "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = '{}' AND pid <> pg_backend_pid()",
        db_name
    );
    let _ = sqlx::query(&terminate_query).execute(&admin_pool).await;

    // Drop the test database
    let drop_db_query = format!("DROP DATABASE IF EXISTS {}", db_name);
    sqlx::query(&drop_db_query).execute(&admin_pool).await?;

    println!("Test database '{}' dropped successfully", db_name);

    admin_pool.close().await;
    Ok(())
}

/// Manual cleanup function to drop the test database
/// Respects KEEP_FIXTURES environment variable
///
/// # Example
/// ```no_run
/// use udex_datastore::integration_test::{data, cleanup_test_database};
///
/// # async fn example() {
/// let data = data(false).await;
/// // ... run tests ...
/// cleanup_test_database(&data.2).await;
/// # }
/// ```
pub async fn cleanup_test_database(db_name: &str) {
    // Check if we should keep fixtures
    let keep_fixtures = std::env::var("KEEP_FIXTURES")
        .map(|v| v.to_lowercase() == "true" || v == "1")
        .unwrap_or(false);

    if keep_fixtures {
        println!("KEEP_FIXTURES=true: Preserving test database '{}'", db_name);
        return;
    }

    if let Err(e) = cleanup_test_database_internal(db_name).await {
        eprintln!("Failed to cleanup test database '{}': {}", db_name, e);
    }
}

// Note: Test database cleanup happens automatically when the test suite completes
// unless KEEP_FIXTURES=true environment variable is set.
// You can also manually call cleanup_test_database() if needed.
