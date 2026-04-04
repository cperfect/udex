use crate::{
    authn::AuthnInterceptor, config::ServerConfig, EntryService, Error, HealthzService,
    IndexService,
};
use std::sync::Arc;
use tonic::transport::Server as TonicServer;
use tonic::transport::{Identity, ServerTlsConfig};
use tonic_middleware::InterceptorFor;
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
    // validate the server configuration
    config.validate()?;

    let addr = config.bind_address;

    // TODO(major): replace println! with tracing - adopt tracing crate for structured, levelled
    // logging throughout the server. Use tracing::info!, tracing::warn!, tracing::error! etc.
    // See: https://docs.rs/tracing
    println!("Initialising services");

    let datastore_arc = Arc::new(datastore);

    let index_service_inner = IndexService::new(datastore_arc.clone());

    let mut entry_service_inner = EntryService::new(datastore_arc.clone());

    // need to do this before moving the inner services below
    println!("Initializing services");

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

    // TODO(major): replace println! with tracing (see above)
    println!("Starting Udex server on {} with TLS", addr);

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
