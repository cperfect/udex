//! Live Hydra integration tests for `udex index delete`.
//!
//! Spins up an in-process udex server, pre-seeds an empty and a non-empty
//! index, then drives the compiled `udex` binary to verify:
//!
//! - Deleting an empty index succeeds (exit 0).
//! - Deleting a non-empty index returns exit 4 (FAILED_PRECONDITION).
//! - Deleting a non-existent index returns exit 2 (NOT_FOUND).
//!
//! Requires: a running Hydra instance (available in the devcontainer compose
//! stack). The Hydra URLs are read from `HYDRA_PUBLIC_URL` / `HYDRA_ADMIN_URL`,
//! defaulting to `http://localhost:4444` / `:4445`.

use std::net::SocketAddr;
use std::sync::OnceLock;

use assert_cmd::Command;
use maybe_once::tokio::{Data, MaybeOnceAsync};
use rstest::*;
use tokio::time::{sleep, Duration};
use tonic::transport::{Certificate, Channel, ClientTlsConfig};
use tonic_health::pb::{health_client::HealthClient, HealthCheckRequest};
use udex_api::index::{HashAlgorithm, Index, IndexUpdate, UpdateIndexRequest};
use udex_datastore::integration_test::init_postgres;
use udex_datastore::{Datastore, Entry};
use udex_test_utils::{bind_file_secret, hydra_admin_url, hydra_public_url, register_hydra_client};

// ── Constants ─────────────────────────────────────────────────────────────────

const BIND_ADDR: &str = "127.0.0.1:50058";
const EMPTY_INDEX_NAME: &str = "cli-hydra-idx-del-empty";
const NON_EMPTY_INDEX_NAME: &str = "cli-hydra-idx-del-nonempty";
const NOT_FOUND_INDEX_NAME: &str = "cli-hydra-idx-del-notfound";
const CLIENT_ID: &str = "cli-hydra-idx-del-client";
const CLIENT_SECRET: &str = "cli-hydra-idx-del-secret";
const AUDIENCE: &str = "cli-hydra-idx-del-audience";

const CA_CERT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../server/tests/certs/ca.crt");
const SERVER_CERT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../server/tests/certs/server.crt"
);
const SERVER_KEY: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../server/tests/certs/server.key"
);

// ── Env helpers ───────────────────────────────────────────────────────────────

fn hydra_issuer() -> String {
    let public = hydra_public_url();
    std::env::var("HYDRA_ISSUER").unwrap_or_else(|_| format!("{}/", public.trim_end_matches('/')))
}

fn token_url() -> String {
    format!("{}/oauth2/token", hydra_public_url())
}

// ── Fixture ───────────────────────────────────────────────────────────────────

type Fixture = ();

fn index_update(description: &str) -> IndexUpdate {
    IndexUpdate {
        description: Some(description.to_string()),
        display_name: Some(description.to_string()),
        max_bulk_operations: Some(100),
        max_key_length: Some(256),
        max_value_length: Some(1024),
        max_kv_pairs_per_context: Some(50),
        hash_algorithm: Some(HashAlgorithm::Xxh3 as i32),
    }
}

async fn init_fixture() -> Fixture {
    udex_server::logging::init_test_tracing();
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let issuer = hydra_issuer();
    let jwks_url = format!(
        "{}/.well-known/jwks.json",
        hydra_public_url().trim_end_matches('/')
    );

    // Fresh isolated database.
    let (datastore, _pool, _db_name) = init_postgres().await;

    // Seed the non-empty index and entry before handing the datastore to the server.
    datastore
        .create_index(Index {
            name: NON_EMPTY_INDEX_NAME.to_string(),
            display_name: "Pre-seeded Non-empty Index".to_string(),
            description: "pre-seeded non-empty index".to_string(),
            max_bulk_operations: 100,
            max_key_length: 256,
            max_value_length: 1024,
            max_kv_pairs_per_context: 50,
            hash_algorithm: HashAlgorithm::Xxh3 as i32,
            created_at: Some(udex_api::now_timestamp()),
            created_by: "fixture".to_string(),
            updated_at: None,
            updated_by: None,
        })
        .await
        .expect("create non-empty index");

    datastore
        .create_entry(Entry {
            key: uuid::Uuid::new_v4(),
            context: udex_api::entry::Context {
                hash: "fixture-seed-hash".to_string(),
                pairs: vec![],
            },
            index_name: NON_EMPTY_INDEX_NAME.to_string(),
        })
        .await
        .expect("seed entry into non-empty index");

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
        init_indexes: vec![
            // Empty index — no entries seeded.
            UpdateIndexRequest {
                name: EMPTY_INDEX_NAME.to_string(),
                update: Some(index_update("empty index for delete test")),
            },
            // Non-empty index already created above; server init will see it exists and skip.
            UpdateIndexRequest {
                name: NON_EMPTY_INDEX_NAME.to_string(),
                update: Some(index_update("non-empty index for delete test")),
            },
        ],
        authz: udex_server::config::AuthzConfig {
            jwks_url: Some(jwks_url),
            jwt_public_key: None,
            jwt_issuer: Some(issuer),
            jwt_audience: Some(AUDIENCE.to_string()),
            danger_allow_non_tls: true,
            scope_claim_name: Some("scp".to_string()),
            mask_subject_in_logs: false,
        },
    };

    tokio::spawn(async move {
        udex_server::server::serve(server_config, datastore)
            .await
            .expect("server failed");
    });

    let ca_pem = std::fs::read(CA_CERT).expect("read CA cert");
    wait_for_server(BIND_ADDR, &ca_pem).await;

    let scopes = format!(
        "udex:index:v1:{EMPTY_INDEX_NAME}:delete \
         udex:index:v1:{NON_EMPTY_INDEX_NAME}:delete \
         udex:index:v1:{NOT_FOUND_INDEX_NAME}:delete"
    );
    register_hydra_client(
        &hydra_admin_url(),
        CLIENT_ID,
        CLIENT_SECRET,
        AUDIENCE,
        &scopes,
    )
    .await;
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
            .connect_timeout(Duration::from_millis(500))
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

pub async fn fixture(serial: bool) -> Data<'static, Fixture> {
    static DATA: OnceLock<MaybeOnceAsync<Fixture>> = OnceLock::new();
    DATA.get_or_init(|| MaybeOnceAsync::new(|| Box::pin(init_fixture())))
        .data(serial)
        .await
}

fn udex() -> Command {
    Command::cargo_bin("udex").expect("udex binary not found")
}

/// Fetch a Hydra token for the delete client with the given scope.
async fn fetch_token(scope: &str) -> String {
    let scope = scope.to_string();
    let output = tokio::task::spawn_blocking(move || {
        Command::cargo_bin("udex")
            .expect("udex binary not found")
            .args([
                "token",
                "fetch",
                "--url",
                &token_url(),
                "--scope",
                &scope,
                "--audience",
                AUDIENCE,
                "--output",
                "json",
            ])
            .env("UDEX_CLIENT_ID", CLIENT_ID)
            .env("UDEX_CLIENT_SECRET", CLIENT_SECRET)
            .output()
            .expect("failed to run token fetch")
    })
    .await
    .expect("spawn_blocking panicked");

    assert!(
        output.status.success(),
        "token fetch failed\nstderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("valid JSON from token fetch");
    json["token"].as_str().expect("token field").to_string()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[rstest]
#[tokio_shared_rt::test]
async fn test_cli_oauth2_index_delete_empty() {
    let _ = fixture(false).await;
    let token = fetch_token(&format!("udex:index:v1:{EMPTY_INDEX_NAME}:delete")).await;

    let output = tokio::task::spawn_blocking(move || {
        udex()
            .args([
                "index",
                "delete",
                EMPTY_INDEX_NAME,
                "--server",
                &format!("https://{BIND_ADDR}"),
                "--ca-cert",
                CA_CERT,
            ])
            .env("UDEX_TOKEN", &token)
            .output()
            .expect("failed to run udex index delete")
    })
    .await
    .expect("spawn_blocking panicked");

    assert!(
        output.status.success(),
        "expected success\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains(EMPTY_INDEX_NAME),
        "expected confirmation message mentioning '{EMPTY_INDEX_NAME}'\nstdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[rstest]
#[tokio_shared_rt::test]
async fn test_cli_oauth2_index_delete_non_empty() {
    let _ = fixture(false).await;
    let token = fetch_token(&format!("udex:index:v1:{NON_EMPTY_INDEX_NAME}:delete")).await;

    let output = tokio::task::spawn_blocking(move || {
        udex()
            .args([
                "index",
                "delete",
                NON_EMPTY_INDEX_NAME,
                "--server",
                &format!("https://{BIND_ADDR}"),
                "--ca-cert",
                CA_CERT,
            ])
            .env("UDEX_TOKEN", &token)
            .output()
            .expect("failed to run udex index delete")
    })
    .await
    .expect("spawn_blocking panicked");

    assert_eq!(
        output.status.code(),
        Some(4),
        "expected exit 4 (FAILED_PRECONDITION) for non-empty index\nstderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[rstest]
#[tokio_shared_rt::test]
async fn test_cli_oauth2_index_delete_not_found() {
    let _ = fixture(false).await;
    let token = fetch_token(&format!("udex:index:v1:{NOT_FOUND_INDEX_NAME}:delete")).await;

    let output = tokio::task::spawn_blocking(move || {
        udex()
            .args([
                "index",
                "delete",
                NOT_FOUND_INDEX_NAME,
                "--server",
                &format!("https://{BIND_ADDR}"),
                "--ca-cert",
                CA_CERT,
            ])
            .env("UDEX_TOKEN", &token)
            .output()
            .expect("failed to run udex index delete")
    })
    .await
    .expect("spawn_blocking panicked");

    assert_eq!(
        output.status.code(),
        Some(2),
        "expected exit 2 (NOT_FOUND) for nonexistent index\nstderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
