use crate::{
    authn::AuthnInterceptor, config::ServerConfig, logging, EntryService, Error, HealthzService,
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

    // Load TLS certificates etc.
    let cert = tokio::fs::read_to_string(config.tls.cert_path)
        .await
        .map_err(|e| Error::ServerError(format!("Failed to read server cert: {}", e)))?;

    let key = tokio::fs::read_to_string(config.tls.key_path)
        .await
        .map_err(|e| Error::ServerError(format!("Failed to read private key: {}", e)))?;

    // construct the tls identity
    // using the cert and key read from the config paths
    let identity = Identity::from_pem(cert, key);

    let auth_interceptor = AuthnInterceptor::new(config.authnz.clone())?;

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
        .map_err(|e| Error::ServerError(format!("Server error: {}", e)))?;

    Ok(())
}
