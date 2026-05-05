//! Live server integration tests for `udex serve`.
//!
//! Exercises the full CLI path: TOML config file → `udex serve` binary →
//! gRPC server with TLS. The binary is spawned as a real child process so
//! that config loading, validation, and the serve command handler are all
//! exercised end-to-end.
//!
//! Requires: `DATABASE_URL` environment variable pointing to a running
//! PostgreSQL instance (same requirement as the datastore integration tests).

use std::process::Command;

use tokio::time::{sleep, Duration};
use tonic::transport::{Certificate, Channel, ClientTlsConfig};
use udex_api::healthz::{healthz_service_client::HealthzServiceClient, HealthzRequest};
use udex_datastore::integration_test::init_postgres;

const BIND_ADDR: &str = "127.0.0.1:50054";
const UDEX_BIN: &str = env!("CARGO_BIN_EXE_udex");

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

/// RAII guard that kills the child process on drop.
struct ServerProcess(std::process::Child);

impl Drop for ServerProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Replace the database name in a `postgres://…/dbname[?params]` URL.
fn replace_db_name(url: &str, db_name: &str) -> String {
    if let Some(slash) = url.rfind('/') {
        let after = &url[slash + 1..];
        let params = after.find('?').map(|i| &after[i..]).unwrap_or("");
        format!("{}/{}{}", &url[..slash], db_name, params)
    } else {
        format!("{}/{}", url, db_name)
    }
}

/// Environment variable used to pass the test database URL to the spawned server process.
const TEST_DB_URL_VAR: &str = "SERVE_TEST_DATABASE_URL";

fn make_config() -> String {
    format!(
        r#"[server]
bind_address = "{BIND_ADDR}"
request_timeout_secs = 30
max_connections = 100
max_message_size_bytes = 4194304

[server.tls]
cert = "urn:secrets-rs:file:{SERVER_CERT}"
key = "urn:secrets-rs:file:{SERVER_KEY}"

[server.authz]
jwt_public_key = "urn:secrets-rs:file:{JWT_PUBLIC_KEY}"
jwt_issuer = "https://cli-serve-test-issuer.example.com"
jwt_audience = "udex-cli-serve-test"

[datastore]
connection_url = "urn:secrets-rs:env:{TEST_DB_URL_VAR}"
max_connections = 5
min_connections = 1
connection_timeout_secs = 10
query_timeout_secs = 30
dangerous_allow_non_tls = true
"#
    )
}

/// Poll healthz over TLS until the server responds or we give up.
async fn wait_for_ready(ca_pem: &str) -> bool {
    for _ in 0..30 {
        sleep(Duration::from_millis(300)).await;
        let tls = ClientTlsConfig::new()
            .ca_certificate(Certificate::from_pem(ca_pem))
            .domain_name("localhost");
        let Ok(ch) = Channel::from_shared(format!("https://{BIND_ADDR}"))
            .unwrap()
            .tls_config(tls)
            .unwrap()
            .connect()
            .await
        else {
            continue;
        };
        if HealthzServiceClient::new(ch)
            .healthz(tonic::Request::new(HealthzRequest {}))
            .await
            .is_ok()
        {
            return true;
        }
    }
    false
}

/// Verifies that `udex serve` starts correctly and serves healthz over TLS.
///
/// The test writes a complete `udex.toml`, spawns the binary, waits for the
/// server to become ready, then asserts the healthz response is healthy.
#[tokio::test]
async fn test_serve_healthz_over_tls() {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    // Create a fresh database with migrations applied.
    let (_, _, db_name) = init_postgres().await;
    let base_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let db_url = replace_db_name(&base_url, &db_name);

    // Write a udex.toml that exercises the full CLI config path.
    let dir = tempfile::tempdir().expect("tempdir");
    let config_path = dir.path().join("udex.toml");
    std::fs::write(&config_path, make_config()).expect("write config");

    // Spawn the real binary — this exercises config loading and the serve handler.
    // The DB URL is passed via env var so the config can use a URN reference
    // (Secret<T>::Deserialize rejects plain postgres:// URLs).
    let child = Command::new(UDEX_BIN)
        .args(["serve", "--config", config_path.to_str().unwrap()])
        .env(TEST_DB_URL_VAR, &db_url)
        .spawn()
        .expect("failed to spawn udex serve");
    let _server = ServerProcess(child);

    // Wait for the server to become ready via TLS healthz.
    let ca_pem = std::fs::read_to_string(CA_CERT).expect("read CA cert");
    assert!(
        wait_for_ready(&ca_pem).await,
        "udex serve did not become ready within the timeout"
    );

    // Connect and assert the healthz response is healthy.
    let tls = ClientTlsConfig::new()
        .ca_certificate(Certificate::from_pem(ca_pem))
        .domain_name("localhost");
    let channel = Channel::from_shared(format!("https://{BIND_ADDR}"))
        .expect("invalid endpoint")
        .tls_config(tls)
        .expect("TLS config error")
        .connect()
        .await
        .expect("failed to connect to server over TLS");

    let body = HealthzServiceClient::new(channel)
        .healthz(tonic::Request::new(HealthzRequest {}))
        .await
        .expect("healthz request failed")
        .into_inner();

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
