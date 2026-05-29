//! Live integration tests for `udex entry lookup-or-create`.
//!
//! Spins up an in-process udex server on a **dedicated** Tokio runtime so that
//! the test threads can call the compiled `udex` binary via blocking `assert_cmd`
//! without starving the server's event loop.
//!
//! Using `tokio_shared_rt` with an in-process server and blocking `assert_cmd`
//! calls deadlocks: the server's connection-handler tasks and the blocking
//! `waitpid` share the same thread pool, so the pool is exhausted before the
//! server can respond.  Running the server on its own `Runtime` (stored in a
//! `OnceLock`) isolates the two completely — the same approach used by
//! `serve_live_tests.rs`, which runs the server as a separate OS process.
//!
//! Requires: `DATABASE_URL` env var pointing to a running PostgreSQL instance.

use std::net::SocketAddr;
use std::sync::OnceLock;

use assert_cmd::Command;
use jsonwebtoken::{encode, EncodingKey, Header};
use predicates::prelude::*;
use time::OffsetDateTime;
use tokio::time::{sleep, Duration};
use tonic::transport::{Certificate, Channel, ClientTlsConfig};
use tonic_health::pb::{health_client::HealthClient, HealthCheckRequest};
use udex_api::index::{CreateIndexRequest, HashAlgorithm};
use udex_datastore::integration_test::init_postgres;
use udex_test_utils::bind_file_secret;

// ── Constants ─────────────────────────────────────────────────────────────────

const BIND_ADDR: &str = "127.0.0.1:50062";
const INDEX_NAME: &str = "cli-entry-loc-test-index";
const JWT_ISSUER: &str = "cli-entry-loc-test-issuer";
const JWT_AUDIENCE: &str = "cli-entry-loc-test-audience";

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
const JWT_PRIVATE_KEY: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../server/tests/jwt/signing_private_key.pem"
);
const UDEX_BIN: &str = env!("CARGO_BIN_EXE_udex");

// ── Server bootstrap ──────────────────────────────────────────────────────────

struct ServerState {
    token: String,
    /// Keeps the server's dedicated runtime alive for the process lifetime.
    _runtime: tokio::runtime::Runtime,
}

/// Returns the signed bearer token for the test index, starting the server on
/// first call. Thread-safe; blocks until the server is ready.
fn server_token() -> &'static str {
    static STATE: OnceLock<ServerState> = OnceLock::new();
    &STATE
        .get_or_init(|| {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("build server runtime");
            let token = rt.block_on(init_server());
            ServerState {
                token,
                _runtime: rt,
            }
        })
        .token
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

async fn init_server() -> String {
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
            name: INDEX_NAME.to_string(),
            display_name: "CLI Test Index".to_string(),
            description: "CLI lookup-or-create test index".to_string(),
            max_bulk_operations: 100,
            max_key_length: 256,
            max_value_length: 1024,
            max_kv_pairs_per_context: 10,
            hash_algorithm: HashAlgorithm::Xxh3 as i32,
        }],
        authz: udex_server::config::AuthzConfig {
            jwks_url: None,
            jwt_public_key: Some(bind_file_secret(JWT_PUBLIC_KEY)),
            jwt_issuer: Some(JWT_ISSUER.to_string()),
            jwt_audience: Some(JWT_AUDIENCE.to_string()),
            danger_allow_non_tls: false,
            scope_claim_name: None,
            mask_subject_in_logs: false,
            jwks_max_failed_refreshes: None,
            jwks_backoff_factor_secs: None,
            jwks_max_age_secs: None,
        },
    };

    tokio::spawn(async move {
        udex_server::server::serve(server_config, datastore)
            .await
            .expect("entry live test server failed");
    });

    let ca_pem = std::fs::read(CA_CERT).expect("read CA cert");
    wait_for_server(BIND_ADDR, &ca_pem).await;

    // Sign a 1-hour JWT with read+write permissions for the test index.
    // `lookup-or-create` requires both: it reads and may create.
    let private_key_pem = std::fs::read_to_string(JWT_PRIVATE_KEY).expect("read JWT private key");
    let signing_key =
        EncodingKey::from_ec_pem(private_key_pem.as_bytes()).expect("EncodingKey from PEM");

    let now = OffsetDateTime::now_utc().unix_timestamp() as usize;
    let claims = udex_api::authz::claims::Claims::new(
        "cli-entry-loc-test-subject".to_string(),
        JWT_ISSUER.to_string(),
        JWT_AUDIENCE.to_string(),
        now + 3600,
        now,
    )
    .with_scope(format!(
        "udex:entry:v1:{INDEX_NAME}:read udex:entry:v1:{INDEX_NAME}:write"
    ));

    let mut header = Header::new(jsonwebtoken::Algorithm::ES256);
    header.typ = Some("JWT".to_string());
    encode(&header, &claims, &signing_key).expect("JWT encode")
}

fn udex_with_auth(token: &str) -> Command {
    let mut cmd = Command::new(UDEX_BIN);
    cmd.env("UDEX_SERVER", format!("https://{BIND_ADDR}"))
        .env("UDEX_CA_CERT", CA_CERT)
        .env("UDEX_TOKEN", token);
    cmd
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// First call for a new context: exit 0, table shows created=true and a UUID key.
#[test]
fn test_cli_entry_lookup_or_create_created_path_table() {
    let token = server_token();

    udex_with_auth(token)
        .args([
            "entry",
            "lookup-or-create",
            INDEX_NAME,
            "--context",
            "cli_loc_user=alice_table",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("key"))
        .stdout(predicate::str::contains("context_hash"))
        .stdout(predicate::str::contains("created"))
        .stdout(predicate::str::contains("true"));
}

/// Second call with the same context: exit 0, table shows created=false,
/// and the returned key matches the first call's key.
#[test]
fn test_cli_entry_lookup_or_create_found_path_table() {
    let token = server_token();

    // First call — creates the entry and captures the key.
    let first_out = udex_with_auth(token)
        .args([
            "entry",
            "lookup-or-create",
            INDEX_NAME,
            "--context",
            "cli_loc_user=bob_table",
        ])
        .output()
        .expect("first lookup-or-create failed to execute");
    assert!(
        first_out.status.success(),
        "first call must succeed\nstderr: {}",
        String::from_utf8_lossy(&first_out.stderr)
    );
    let first_stdout = String::from_utf8_lossy(&first_out.stdout);
    assert!(
        first_stdout.contains("true"),
        "first call must show created=true\nstdout: {first_stdout}"
    );

    // Extract the key value from the table output.
    let first_key = first_stdout
        .lines()
        .find(|l| l.contains("| key") || (l.contains("key") && !l.contains("context_hash")))
        .and_then(|l| l.split('|').nth(2))
        .map(|s| s.trim().to_string())
        .expect("could not extract key from first call output");

    // Second call — same context, entry exists.
    udex_with_auth(token)
        .args([
            "entry",
            "lookup-or-create",
            INDEX_NAME,
            "--context",
            "cli_loc_user=bob_table",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(&first_key))
        .stdout(predicate::str::contains("false"));
}

/// JSON output format: created field is present and boolean.
#[test]
fn test_cli_entry_lookup_or_create_json_output() {
    let token = server_token();

    let out = udex_with_auth(token)
        .args([
            "--output",
            "json",
            "entry",
            "lookup-or-create",
            INDEX_NAME,
            "--context",
            "cli_loc_user=carol_json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&out).expect("output must be valid JSON");
    assert!(
        json.get("key").and_then(|v| v.as_str()).is_some(),
        "JSON must contain a string key"
    );
    assert!(
        json.get("context_hash").and_then(|v| v.as_str()).is_some(),
        "JSON must contain a string context_hash"
    );
    assert_eq!(
        json.get("created").and_then(|v| v.as_bool()),
        Some(true),
        "JSON created must be true for a new entry"
    );
}

/// YAML output format: created field is present.
#[test]
fn test_cli_entry_lookup_or_create_yaml_output() {
    let token = server_token();

    udex_with_auth(token)
        .args([
            "--output",
            "yaml",
            "entry",
            "lookup-or-create",
            INDEX_NAME,
            "--context",
            "cli_loc_user=dan_yaml",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("created: true"))
        .stdout(predicate::str::contains("key:"))
        .stdout(predicate::str::contains("context_hash:"));
}
