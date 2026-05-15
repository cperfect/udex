use crate::{
    authz::AuthzInterceptor, config::ServerConfig, logging, EntryService, Error, HealthzService,
    IndexService,
};
use std::sync::Arc;
use tonic::transport::Server as TonicServer;
use tonic::transport::{Identity, ServerTlsConfig};
use tonic_middleware::InterceptorFor;
use tower_http::trace::TraceLayer;
use udex_api::{
    authz::{entry::EntryServiceAuthorizor, index::IndexServiceAuthorizor},
    entry::entry_service_server::EntryServiceServer,
    healthz::healthz_service_server::HealthzServiceServer,
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
    apply_and_check_migrations(&*datastore, apply_migrations).await?;
    serve(server_config, *datastore).await
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

    let index_service_inner = IndexService::new(datastore_arc.clone());

    let mut entry_service_inner = EntryService::new(datastore_arc.clone());

    index_service_inner
        .init(config.init_indexes.clone())
        .await?;

    let index_service_inner_arc = Arc::new(index_service_inner);

    entry_service_inner
        .init(index_service_inner_arc.clone())
        .await?;

    let entry_service_inner_arc = Arc::new(entry_service_inner);

    let entry_service = EntryServiceAuthorizor::new(entry_service_inner_arc.clone());

    let index_service = IndexServiceAuthorizor::new(index_service_inner_arc.clone());

    // which we need to consume in the server
    let healthz_service = HealthzService::new(
        entry_service_inner_arc.clone(),
        index_service_inner_arc.clone(),
    );

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

    let auth_interceptor = AuthzInterceptor::new(config.authz)?;

    let entry_server = EntryServiceServer::new(entry_service);
    let index_server = IndexServiceServer::new(index_service);
    let healthz_service = HealthzServiceServer::new(healthz_service); // healthz is not authenticated

    // Build and start the server with TLS
    TonicServer::builder()
        .layer(TraceLayer::new_for_grpc())
        .tls_config(ServerTlsConfig::new().identity(identity))
        .map_err(|e| Error::ServerError(format!("TLS configuration error: {}", e)))?
        .add_service(InterceptorFor::new(index_server, auth_interceptor.clone()))
        .add_service(InterceptorFor::new(entry_server, auth_interceptor.clone()))
        .add_service(healthz_service)
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
