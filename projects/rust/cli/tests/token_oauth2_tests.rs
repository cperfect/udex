//! Live Hydra integration tests for `udex token` subcommands.
//!
//! Spins up an in-process udex server configured to validate JWTs via Hydra's
//! JWKS endpoint, registers a Hydra OAuth2 client, then drives the compiled
//! `udex` binary with `assert_cmd` to cover every `token fetch` scenario and
//! one end-to-end API call.
//!
//! Requires: a running Hydra instance (available in the devcontainer compose
//! stack). The Hydra URLs are read from `HYDRA_PUBLIC_URL` / `HYDRA_ADMIN_URL`
//! / `HYDRA_ISSUER`, defaulting to `http://localhost:4444` / `:4445`.

use std::net::SocketAddr;
use std::sync::OnceLock;

use assert_cmd::Command;
use maybe_once::tokio::{Data, MaybeOnceAsync};
use predicates::prelude::*;
use rstest::*;
use tokio::time::{sleep, Duration};
use tonic::transport::{Certificate, Channel, ClientTlsConfig};
use tonic_health::pb::{health_client::HealthClient, HealthCheckRequest};
use udex_api::index::{CreateIndexRequest, HashAlgorithm};
use udex_datastore::integration_test::init_postgres;
use udex_test_utils::{bind_file_secret, hydra_admin_url, hydra_public_url, register_hydra_client};

// ── Constants ─────────────────────────────────────────────────────────────────

const BIND_ADDR: &str = "127.0.0.1:50057";
const INDEX_NAME: &str = "cli-hydra-test-index";
const CLIENT_ID: &str = "cli-hydra-test-client";
const CLIENT_SECRET: &str = "cli-hydra-test-secret";
const AUDIENCE: &str = "cli-hydra-test-audience";

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

fn token_url() -> String {
    format!("{}/oauth2/token", hydra_public_url())
}

// ── Fixture ───────────────────────────────────────────────────────────────────

type Fixture = ();

async fn init_fixture() -> Fixture {
    udex_server::logging::init_test_tracing();
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let public_url = hydra_public_url();
    let admin_url = hydra_admin_url();
    let issuer = std::env::var("HYDRA_ISSUER")
        .unwrap_or_else(|_| format!("{}/", public_url.trim_end_matches('/')));
    let jwks_url = format!("{public_url}/.well-known/jwks.json");

    let datastore_fixtures = init_postgres().await;
    let datastore = datastore_fixtures.0;

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
        observability: None,
        init_indexes: vec![CreateIndexRequest {
            name: INDEX_NAME.to_string(),
            display_name: INDEX_NAME.to_string(),
            description: INDEX_NAME.to_string(),
            max_bulk_operations: 100,
            max_key_length: 256,
            max_value_length: 1024,
            max_kv_pairs_per_context: 50,
            hash_algorithm: HashAlgorithm::Xxh3 as i32,
        }],
        authz: udex_server::config::AuthzConfig {
            jwks_url: Some(jwks_url),
            jwt_public_key: None,
            jwt_issuer: Some(issuer),
            // The Hydra client is registered with AUDIENCE so Hydra includes it
            // in every issued JWT automatically — no explicit audience request needed.
            jwt_audience: Some(AUDIENCE.to_string()),
            danger_allow_non_tls: true,
            scope_claim_name: Some("scp".to_string()),
            mask_subject_in_logs: false,
            jwks_max_failed_refreshes: None,
            jwks_backoff_factor_secs: None,
            jwks_max_age_secs: None,
        },
    };

    let ca_pem = std::fs::read(CA_CERT).expect("read CA cert");

    tokio::spawn(async move {
        udex_server::server::serve(server_config, datastore)
            .await
            .expect("server failed");
    });

    wait_for_server(BIND_ADDR, &ca_pem).await;

    let scopes = format!(
        "udex:index:v1:list \
         udex:index:v1:{INDEX_NAME}:read \
         udex:entry:v1:{INDEX_NAME}:create \
         udex:entry:v1:{INDEX_NAME}:read \
         udex:entry:v1:{INDEX_NAME}:write \
         udex:entry:v1:{INDEX_NAME}:delete"
    );
    register_hydra_client(&admin_url, CLIENT_ID, CLIENT_SECRET, AUDIENCE, &scopes).await;
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

// ── token fetch ───────────────────────────────────────────────────────────────

#[rstest]
#[tokio_shared_rt::test]
async fn test_cli_oauth2_token_fetch_table_output() {
    let _ = fixture(false).await;

    udex()
        .args(["token", "fetch", "--url", &token_url()])
        .env("UDEX_CLIENT_ID", CLIENT_ID)
        .env("UDEX_CLIENT_SECRET", CLIENT_SECRET)
        .assert()
        .success()
        .stdout(predicate::str::contains("Header"))
        .stdout(predicate::str::contains("Claims"));
}

#[rstest]
#[tokio_shared_rt::test]
async fn test_cli_oauth2_token_fetch_json_output() {
    let _ = fixture(false).await;

    let output = udex()
        .args(["token", "fetch", "--url", &token_url(), "--output", "json"])
        .env("UDEX_CLIENT_ID", CLIENT_ID)
        .env("UDEX_CLIENT_SECRET", CLIENT_SECRET)
        .output()
        .expect("failed to run command");

    assert!(
        output.status.success(),
        "expected success\nstderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("expected valid JSON output");
    assert!(
        json["token"].as_str().is_some_and(|s| !s.is_empty()),
        "token field missing or empty"
    );
    assert!(json.get("header").is_some(), "header field missing");
    assert!(json.get("claims").is_some(), "claims field missing");
}

#[rstest]
#[tokio_shared_rt::test]
async fn test_cli_oauth2_token_fetch_yaml_output() {
    let _ = fixture(false).await;

    udex()
        .args(["token", "fetch", "--url", &token_url(), "--output", "yaml"])
        .env("UDEX_CLIENT_ID", CLIENT_ID)
        .env("UDEX_CLIENT_SECRET", CLIENT_SECRET)
        .assert()
        .success()
        .stdout(predicate::str::contains("token:"))
        .stdout(predicate::str::contains("header:"))
        .stdout(predicate::str::contains("claims:"));
}

#[rstest]
#[tokio_shared_rt::test]
async fn test_cli_oauth2_token_fetch_missing_client_id() {
    let _ = fixture(false).await;

    udex()
        .args(["token", "fetch", "--url", &token_url()])
        .env_remove("UDEX_CLIENT_ID")
        .env("UDEX_CLIENT_SECRET", CLIENT_SECRET)
        .assert()
        .failure()
        .stderr(predicate::str::contains("UDEX_CLIENT_ID"));
}

#[rstest]
#[tokio_shared_rt::test]
async fn test_cli_oauth2_token_fetch_missing_client_secret() {
    let _ = fixture(false).await;

    udex()
        .args(["token", "fetch", "--url", &token_url()])
        .env("UDEX_CLIENT_ID", CLIENT_ID)
        .env_remove("UDEX_CLIENT_SECRET")
        .assert()
        .failure()
        .stderr(predicate::str::contains("UDEX_CLIENT_SECRET"));
}

#[rstest]
#[tokio_shared_rt::test]
async fn test_cli_oauth2_token_fetch_wrong_credentials() {
    let _ = fixture(false).await;

    udex()
        .args(["token", "fetch", "--url", &token_url()])
        .env("UDEX_CLIENT_ID", CLIENT_ID)
        .env("UDEX_CLIENT_SECRET", "this-is-the-wrong-secret")
        .assert()
        .failure()
        .stderr(predicate::str::contains("token request failed"));
}

// ── end-to-end: fetch token → call API ───────────────────────────────────────

#[rstest]
#[tokio_shared_rt::test]
async fn test_cli_oauth2_token_fetch_and_use_for_index_list() {
    let _ = fixture(false).await;

    // Fetch a token with the list scope as JSON so we can extract the raw JWT.
    let output = udex()
        .args([
            "token",
            "fetch",
            "--url",
            &token_url(),
            "--scope",
            "udex:index:v1:list",
            "--audience",
            AUDIENCE,
            "--output",
            "json",
        ])
        .env("UDEX_CLIENT_ID", CLIENT_ID)
        .env("UDEX_CLIENT_SECRET", CLIENT_SECRET)
        .output()
        .expect("failed to run token fetch");

    assert!(
        output.status.success(),
        "token fetch failed\nstderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("valid JSON from token fetch");
    let token = json["token"].as_str().expect("token field").to_string();

    // Use the fetched JWT to call the live server. spawn_blocking is required:
    // tokio-shared-rt uses a current_thread runtime, so blocking here would
    // starve the server task and deadlock — the subprocess would wait for a
    // response the server can never send.
    let output = tokio::task::spawn_blocking(move || {
        udex()
            .args([
                "index",
                "list",
                "--server",
                &format!("https://{BIND_ADDR}"),
                "--ca-cert",
                CA_CERT,
            ])
            .env("UDEX_TOKEN", &token)
            .output()
            .expect("failed to run udex index list")
    })
    .await
    .expect("spawn_blocking panicked");

    assert!(
        output.status.success(),
        "index list failed\nstderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains(INDEX_NAME),
        "expected '{INDEX_NAME}' in output\nstdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}
