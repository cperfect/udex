//! Live server integration tests for `udex serve`.
//!
//! Spins up a real gRPC server (with TLS and a live PostgreSQL database) and
//! asserts that the healthz endpoint responds correctly over TLS.
//!
//! Requires: `DATABASE_URL` environment variable pointing to a running
//! PostgreSQL instance (same requirement as the datastore integration tests).

use std::net::SocketAddr;
use std::sync::OnceLock;

use maybe_once::tokio::{Data, MaybeOnceAsync};
use tokio::time::{sleep, Duration};
use tonic::transport::{Certificate, Channel, ClientTlsConfig};
use udex_api::healthz::{healthz_service_client::HealthzServiceClient, HealthzRequest};
use udex_datastore::integration_test::init_postgres;
use udex_server::{
    config::{AuthNzConfig, ServerConfig, TlsConfig},
    logging, server,
};

const BIND_ADDR: &str = "127.0.0.1:50054";

// Absolute paths to the shared test PKI, resolved at compile time relative to
// this crate's manifest directory.
const SERVER_CERT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../server/tests/certs/server.crt"
);
const SERVER_KEY: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../server/tests/certs/server.key"
);
const CA_CERT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../server/tests/certs/ca.crt");
const JWT_PUBLIC_KEY: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../server/tests/jwt/signing_public_key.pem"
);

type FixtureType = (
    ServerConfig,
    tokio::task::JoinHandle<()>,
    String, // db_name — kept alive for automatic cleanup on process exit
);

async fn init_server() -> FixtureType {
    logging::init_test_tracing();
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let (datastore, _, db_name) = init_postgres().await;

    let bind_address: SocketAddr = BIND_ADDR.parse().expect("valid bind address");

    let server_config = ServerConfig {
        bind_address,
        tls: TlsConfig {
            cert_path: SERVER_CERT.to_string(),
            key_path: SERVER_KEY.to_string(),
        },
        authnz: AuthNzConfig {
            jwt_public_key_path: Some(JWT_PUBLIC_KEY.to_string()),
            jwt_issuer: Some("https://cli-serve-test-issuer.example.com".to_string()),
            jwt_audience: Some("udex-cli-serve-test".to_string()),
        },
        ..ServerConfig::default()
    };

    let server_config_clone = server_config.clone();
    let handle = tokio::spawn(async move {
        server::serve(server_config_clone, datastore)
            .await
            .expect("server failed");
    });

    sleep(Duration::from_millis(200)).await;

    (server_config, handle, db_name)
}

async fn fixture(serial: bool) -> Data<'static, FixtureType> {
    static DATA: OnceLock<MaybeOnceAsync<FixtureType>> = OnceLock::new();
    DATA.get_or_init(|| MaybeOnceAsync::new(|| Box::pin(init_server())))
        .data(serial)
        .await
}

/// Verifies that the server started by `udex serve` responds to healthz
/// requests over TLS, using the CA certificate from the test PKI.
#[tokio_shared_rt::test]
async fn test_serve_healthz_over_tls() {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let data = fixture(false).await;
    let bind_address = data.0.bind_address;

    let ca_pem = tokio::fs::read_to_string(CA_CERT)
        .await
        .expect("failed to read CA cert");

    let tls = ClientTlsConfig::new()
        .ca_certificate(Certificate::from_pem(ca_pem))
        .domain_name("localhost");

    let channel = Channel::from_shared(format!("https://{}", bind_address))
        .expect("invalid endpoint")
        .tls_config(tls)
        .expect("TLS config error")
        .connect()
        .await
        .expect("failed to connect to server over TLS");

    let mut client = HealthzServiceClient::new(channel);
    let response = client
        .healthz(tonic::Request::new(HealthzRequest {}))
        .await
        .expect("healthz request failed");

    let body = response.into_inner();
    assert!(
        body.is_healthy,
        "server reported unhealthy: {:?}",
        body.status_messages
    );
    assert!(
        body.server_time.is_some(),
        "server_time missing from healthz response"
    );
}
