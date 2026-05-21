//! Integration tests for `udex health`.
//!
//! Spins up an in-process udex server on a dedicated Tokio runtime (same
//! pattern as `entry_live_tests.rs`) so that blocking `assert_cmd` calls do
//! not starve the server's event loop.
//!
//! Two tests are provided:
//!
//! - `test_cli_health_serving` — server is up, expects exit 0 and "SERVING".
//! - `test_cli_health_unreachable` — nothing listening on the target port,
//!   expects exit 8 (transport failure).
//!
//! Requires: `DATABASE_URL` env var pointing to a running PostgreSQL instance.

use std::net::SocketAddr;
use std::sync::OnceLock;

use assert_cmd::Command;
use predicates::prelude::*;
use tokio::time::{sleep, Duration};
use tonic::transport::{Certificate, Channel, ClientTlsConfig};
use tonic_health::pb::{health_client::HealthClient, HealthCheckRequest};
use udex_api::index::{CreateIndexRequest, HashAlgorithm};
use udex_datastore::integration_test::init_postgres;
use udex_test_utils::bind_file_secret;

// ── Constants ─────────────────────────────────────────────────────────────────

const BIND_ADDR: &str = "127.0.0.1:50063";

const CA_CERT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../server/tests/certs/ca.crt");
const SERVER_CERT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../server/tests/certs/server.crt"
);
const SERVER_KEY: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../server/tests/certs/server.key"
);
const JWT_PUBLIC_KEY: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../server/tests/jwt/signing_public_key.pem"
);

// ── Server bootstrap ──────────────────────────────────────────────────────────

struct ServerState {
    /// Keeps the server's dedicated runtime alive for the process lifetime.
    _runtime: tokio::runtime::Runtime,
}

fn start_server() -> &'static ServerState {
    static STATE: OnceLock<ServerState> = OnceLock::new();
    STATE.get_or_init(|| {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("build server runtime");
        rt.block_on(init_server());
        ServerState { _runtime: rt }
    })
}

async fn wait_for_server(addr: &str, ca_pem: &[u8]) {
    let tls = ClientTlsConfig::new()
        .ca_certificate(Certificate::from_pem(ca_pem))
        .domain_name("localhost");
    for _ in 0..30 {
        sleep(Duration::from_millis(100)).await;
        let Ok(ch) = Channel::from_shared(format!("https://{addr}"))
            .unwrap()
            .tls_config(tls.clone())
            .unwrap()
            .connect()
            .await
        else {
            continue;
        };
        if HealthClient::new(ch)
            .check(HealthCheckRequest {
                service: "".to_string(),
            })
            .await
            .is_ok()
        {
            return;
        }
    }
    panic!("server at {addr} did not become ready within 3 seconds");
}

async fn init_server() {
    udex_server::logging::init_test_tracing();
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let (datastore, _pool, _db_name) = init_postgres().await;
    let bind_address: SocketAddr = BIND_ADDR.parse().unwrap();

    let server_config = udex_server::config::ServerConfig {
        bind_address,
        request_timeout: std::time::Duration::from_secs(30),
        max_connections: 1000,
        max_message_size: 4 * 1024 * 1024,
        tls: udex_server::config::TlsConfig {
            cert: bind_file_secret(SERVER_CERT),
            key: bind_file_secret(SERVER_KEY),
        },
        init_indexes: vec![CreateIndexRequest {
            name: "cli-health-test-index".to_string(),
            display_name: "CLI health test index".to_string(),
            description: "CLI health test index".to_string(),
            max_bulk_operations: 100,
            max_key_length: 256,
            max_value_length: 1024,
            max_kv_pairs_per_context: 50,
            hash_algorithm: HashAlgorithm::Xxh3 as i32,
        }],
        authz: udex_server::config::AuthzConfig {
            jwks_url: None,
            jwt_public_key: Some(bind_file_secret(JWT_PUBLIC_KEY)),
            jwt_issuer: Some("cli-health-test-issuer".to_string()),
            jwt_audience: Some("cli-health-test-audience".to_string()),
            danger_allow_non_tls: false,
            scope_claim_name: None,
            mask_subject_in_logs: false,
        },
    };

    tokio::spawn(async move {
        udex_server::server::serve(server_config, datastore)
            .await
            .expect("health test server failed");
    });

    let ca_pem = std::fs::read(CA_CERT).expect("read CA cert");
    wait_for_server(BIND_ADDR, &ca_pem).await;
}

fn udex() -> Command {
    Command::cargo_bin("udex").expect("udex binary not found")
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[test]
fn test_cli_health_serving() {
    start_server();

    udex()
        .args([
            "health",
            "--server",
            &format!("https://{BIND_ADDR}"),
            "--ca-cert",
            CA_CERT,
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("SERVING"));
}

#[test]
fn test_cli_health_unreachable() {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    udex()
        .args([
            "health",
            "--server",
            "https://127.0.0.1:59998",
            "--ca-cert",
            CA_CERT,
        ])
        .assert()
        .code(8);
}
