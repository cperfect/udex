use crate::{
    authz::AuthzInterceptor, config::ServerConfig, logging, EntryService, Error, IndexService,
};
use std::sync::Arc;
use tonic::transport::Server as TonicServer;
use tonic::transport::{Identity, ServerTlsConfig};
use tonic_health::ServingStatus;
use tonic_middleware::InterceptorFor;
use tower_http::trace::TraceLayer;
use udex_api::{
    authz::{entry::EntryServiceAuthorizor, index::IndexServiceAuthorizor},
    entry::entry_service_server::EntryServiceServer,
    index::index_service_server::IndexServiceServer,
};
use udex_datastore::Datastore;
use udex_datastore::{config::DatastoreConfig, postgres::PostgresDatastore, Migrator};

/// Initialises the PostgreSQL datastore, conditionally applies migrations, enforces
/// schema version, and starts the server.
///
/// This is the primary entry point for production use. For tests, use [`serve`]
/// directly with a pre-built datastore.
pub async fn start(
    server_config: ServerConfig,
    datastore_config: DatastoreConfig,
) -> Result<(), Error> {
    let apply_migrations = datastore_config.apply_migrations;
    let datastore = PostgresDatastore::init(datastore_config)
        .await
        .map_err(Error::Datastore)?;
    apply_and_check_migrations(&datastore, apply_migrations).await?;
    serve(server_config, datastore).await
}

/// Conditionally runs migrations then enforces the schema version.
///
/// If `apply_migrations` is true, outstanding migrations are applied first.
/// The version check always runs afterwards; a mismatch is logged at ERROR
/// level and returned as an error so the caller can abort startup.
async fn apply_and_check_migrations(
    migrator: &dyn Migrator,
    apply_migrations: bool,
) -> Result<(), Error> {
    if apply_migrations {
        tracing::info!("Applying database migrations");
        migrator.migrate().await.map_err(Error::Datastore)?;
    }
    migrator.check_migration_version().await.map_err(|e| {
        tracing::error!(
            error = %e,
            apply_migrations,
            "Database schema version mismatch — server cannot start; \
             run `udex migrate apply` or set apply_migrations=true to resolve"
        );
        Error::Datastore(e)
    })
}

/// Starts the Udex server with the provided configuration and datastore.
pub async fn serve<D>(config: ServerConfig, datastore: D) -> Result<(), Error>
where
    D: Datastore + Send + Sync + 'static,
{
    logging::init_tracing();

    // validate the server configuration
    config.validate()?;

    let addr = config.bind_address;

    tracing::info!("Initializing services");

    let datastore_arc = Arc::new(datastore);

    let (health_reporter, health_service) = tonic_health::server::health_reporter();

    // Register all services as NOT_SERVING up front. The gRPC health protocol
    // distinguishes NOT_FOUND ("unknown service") from NOT_SERVING ("known but not
    // ready"); a client polling during startup should see the latter. In practice
    // this window cannot be observed — the gRPC port is not open until serve() is
    // called, which is after all init — but we register early for protocol
    // correctness. Each named service transitions to SERVING once its init()
    // completes; "" transitions to SERVING only after all init (including the JWKS
    // fetch) is done.
    health_reporter
        .set_service_status("", ServingStatus::NotServing)
        .await;
    health_reporter
        .set_service_status("udex.index.v1.IndexService", ServingStatus::NotServing)
        .await;
    health_reporter
        .set_service_status("udex.entry.v1.EntryService", ServingStatus::NotServing)
        .await;

    let index_service_inner =
        IndexService::new(Arc::clone(&datastore_arc), health_reporter.clone());

    let entry_service_inner =
        EntryService::new(Arc::clone(&datastore_arc), health_reporter.clone());

    index_service_inner
        .init(config.init_indexes.clone())
        .await?;

    // Pass &dyn IndexService — init only needs a transient reference to list
    // existing indices; no shared ownership is required.
    entry_service_inner.init(&index_service_inner).await?;

    // The authorizors take ownership: they are the sole owners of their inner
    // service and never share or clone it. Tonic wraps the authorizor itself in
    // Arc<Inner> to serve concurrent requests.
    let entry_service = EntryServiceAuthorizor::new(entry_service_inner);

    let index_service = IndexServiceAuthorizor::new(index_service_inner);

    tracing::info!(addr = %addr, "Starting Udex server with TLS");

    let cert_pem = config
        .tls
        .cert
        .value()
        .map_err(|_| Error::ConfigValidation("tls.cert is not bound".to_string()))?
        .clone();
    let key_pem = config
        .tls
        .key
        .value()
        .map_err(|_| Error::ConfigValidation("tls.key is not bound".to_string()))?
        .clone();
    let identity = Identity::from_pem(cert_pem, key_pem);

    let auth_interceptor = AuthzInterceptor::new(config.authz).await?;

    // Mark the overall server as SERVING only after all init is complete: both service
    // init() calls and the JWKS fetch in AuthzInterceptor::new(). This ensures the ""
    // health entry is never SERVING while the server is still initialising.
    health_reporter
        .set_service_status("", ServingStatus::Serving)
        .await;

    let entry_server = EntryServiceServer::new(entry_service);
    let index_server = IndexServiceServer::new(index_service);

    // Build and start the server with TLS
    TonicServer::builder()
        .layer(TraceLayer::new_for_grpc())
        .tls_config(ServerTlsConfig::new().identity(identity))
        .map_err(|e| Error::ServerError(format!("TLS configuration error: {}", e)))?
        .add_service(InterceptorFor::new(index_server, auth_interceptor.clone()))
        .add_service(InterceptorFor::new(entry_server, auth_interceptor.clone()))
        .add_service(health_service)
        .serve(addr)
        .await
        .map_err(|e| Error::ServerError(format!("Server error: {e:?}")))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
    use udex_datastore::{Error as DatastoreError, Migrator};

    struct MockMigrator {
        current: AtomicI64,
        latest: i64,
        migrate_called: AtomicBool,
    }

    impl MockMigrator {
        fn new(current: i64, latest: i64) -> Self {
            Self {
                current: AtomicI64::new(current),
                latest,
                migrate_called: AtomicBool::new(false),
            }
        }
    }

    #[tonic::async_trait]
    impl Migrator for MockMigrator {
        async fn migrate(&self) -> Result<(), DatastoreError> {
            self.migrate_called.store(true, Ordering::SeqCst);
            self.current.store(self.latest, Ordering::SeqCst);
            Ok(())
        }
        async fn current_version(&self) -> Result<i64, DatastoreError> {
            Ok(self.current.load(Ordering::SeqCst))
        }
        async fn latest_version(&self) -> Result<i64, DatastoreError> {
            Ok(self.latest)
        }
    }

    #[tokio::test]
    async fn migration_apply_false_db_current_ok() {
        let m = MockMigrator::new(1, 1);
        assert!(apply_and_check_migrations(&m, false).await.is_ok());
        assert!(
            !m.migrate_called.load(Ordering::SeqCst),
            "migrate must not be called when apply_migrations=false"
        );
    }

    #[tokio::test]
    async fn migration_apply_false_db_behind_fails() {
        let m = MockMigrator::new(0, 1);
        assert!(apply_and_check_migrations(&m, false).await.is_err());
        assert!(
            !m.migrate_called.load(Ordering::SeqCst),
            "migrate must not be called when apply_migrations=false"
        );
    }

    #[tokio::test]
    async fn migration_apply_true_db_behind_migrates_and_succeeds() {
        let m = MockMigrator::new(0, 1);
        assert!(apply_and_check_migrations(&m, true).await.is_ok());
        assert!(
            m.migrate_called.load(Ordering::SeqCst),
            "migrate must be called when apply_migrations=true"
        );
    }

    #[tokio::test]
    async fn migration_apply_true_db_current_still_calls_migrate() {
        let m = MockMigrator::new(1, 1);
        assert!(apply_and_check_migrations(&m, true).await.is_ok());
        assert!(
            m.migrate_called.load(Ordering::SeqCst),
            "migrate must be called even when db is already current"
        );
    }
}
