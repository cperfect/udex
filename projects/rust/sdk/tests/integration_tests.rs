//! Integration tests for `udex-sdk`.
//!
//! Three fixtures are provided:
//!
//! 1. **JWT fixture** (`data()`) — always runs. Spins up an embedded server
//!    with static-JWT auth and connects an SDK client via
//!    [`ClientOptions::static_bearer_token`].
//!
//! 2. **Hydra fixture** (`data_hydra()`) — requires a running Hydra instance
//!    (set by `HYDRA_ADMIN_URL` / `HYDRA_PUBLIC_URL` in `.env`). Connects the
//!    SDK using [`ClientOptions::client_credentials`] so the full
//!    `TokenManager` lifecycle is exercised.
//!
//! 3. **k8s fixture** (`data_k8s()`) — requires a live k3d cluster with udex
//!    deployed (set `K8S_SERVER_URL`, e.g. `https://localhost:8443`). Skipped
//!    silently when `K8S_SERVER_URL` is not set. Uses the same Hydra instance
//!    as the Hydra fixture for OAuth2 token acquisition.

use std::io::Write;
use std::net::SocketAddr;
use std::sync::OnceLock;

use jsonwebtoken::{encode, EncodingKey, Header};
use maybe_once::tokio::{Data, MaybeOnceAsync};
use rstest::*;
use time::OffsetDateTime;
use tokio::time::{sleep, Duration};
use tonic::transport::{Certificate, Channel, ClientTlsConfig};
use tonic_health::pb::{health_client::HealthClient, HealthCheckRequest};
use udex_api::index::{CreateIndexRequest, HashAlgorithm};
use udex_datastore::integration_test::init_postgres;
use udex_sdk::{ClientOptions, ContextInput, HealthStatus, KeyValuePair, UdexClient, Value};
use udex_test_utils::{bind_file_secret, hydra_admin_url, hydra_public_url, register_hydra_client};

// ── Port constants ────────────────────────────────────────────────────────────
// Different from the server crate's own tests to avoid port conflicts when both
// run concurrently.

const SDK_JWT_BIND_ADDR: &str = "127.0.0.1:50054";
const SDK_HYDRA_BIND_ADDR: &str = "127.0.0.1:15055";
const ID_PREFIX: &str = "sdk-integration-test";
const ID_HYDRA_PREFIX: &str = "sdk-hydra-integration-test";
const ID_K8S_PREFIX: &str = "sdk-k8s-integration-test";

// The dev Helm chart defaults to 2 server replicas (values.yaml replicaCount).
// The multi-instance k8s tests rely on there being exactly this many addressable
// pods so they can pin requests to specific instances. See ST0025.
const K8S_REPLICAS: usize = 2;

// ── Cert / JWT key paths ──────────────────────────────────────────────────────
// CARGO_MANIFEST_DIR points to the sdk/ package root at compile time.
// The server's test fixtures live one level up under server/tests/.

const CERTS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../server/tests/certs");
const JWT_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../server/tests/jwt");
// Traefik edge certs (k8s only). With TLS terminated at Traefik, clients now
// validate THIS cert/CA, not the pod's (CERTS_DIR). See ST0024.
const EDGE_CERTS_DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../projects/k8s/traefik/certs"
);

fn server_cert_path(file: &str) -> String {
    format!("{CERTS_DIR}/{file}")
}

fn edge_cert_path(file: &str) -> String {
    format!("{EDGE_CERTS_DIR}/{file}")
}

fn jwt_key_path(file: &str) -> String {
    format!("{JWT_DIR}/{file}")
}

// ── Server readiness ──────────────────────────────────────────────────────────

/// Poll the healthz endpoint over TLS until the server responds or 3 seconds elapse.
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

// ── JWT fixture ───────────────────────────────────────────────────────────────

type JwtFixture = (
    UdexClient,
    String, // index name
);

async fn init_jwt_fixture() -> JwtFixture {
    udex_server::logging::init_test_tracing();
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let datastore_fixtures = init_postgres().await;
    let datastore = datastore_fixtures.0;

    let index_name = format!("{ID_PREFIX}-index");
    let jwt_issuer = format!("{ID_PREFIX}-issuer");
    let jwt_audience = format!("{ID_PREFIX}-audience");
    let bind_address: SocketAddr = SDK_JWT_BIND_ADDR.parse().unwrap();

    let server_config = udex_server::config::ServerConfig {
        bind_address,
        request_timeout: std::time::Duration::from_secs(30),
        max_connections: 1000,
        max_message_size: 4 * 1024 * 1024,
        tls: udex_server::config::TlsConfig {
            cert: bind_file_secret(&server_cert_path("server.crt")),
            key: bind_file_secret(&server_cert_path("server.key")),
        },
        observability: None,
        init_indexes: vec![CreateIndexRequest {
            name: index_name.clone(),
            display_name: index_name.clone(),
            description: index_name.clone(),
            max_bulk_operations: 100,
            max_key_length: 256,
            max_value_length: 1024,
            max_kv_pairs_per_context: 50,
            hash_algorithm: HashAlgorithm::Xxh3 as i32,
        }],
        authz: udex_server::config::AuthzConfig {
            jwks_url: None,
            jwt_public_key: Some(bind_file_secret(&jwt_key_path("signing_public_key.pem"))),
            jwt_issuer: Some(jwt_issuer.clone()),
            jwt_audience: Some(jwt_audience.clone()),
            danger_allow_non_tls: false,
            scope_claim_name: None,
            mask_subject_in_logs: false,
            jwks_max_failed_refreshes: None,
            jwks_backoff_factor_secs: None,
            jwks_max_age_secs: None,
        },
    };

    let ca_pem = tokio::fs::read(server_cert_path("ca.crt"))
        .await
        .expect("read CA cert");

    tokio::spawn(async move {
        udex_server::server::serve(server_config, datastore)
            .await
            .expect("server failed");
    });

    wait_for_server(SDK_JWT_BIND_ADDR, &ca_pem).await;

    // Sign a JWT for the test index using `make_token`'s default settings:
    // a 1-hour lifetime and the helper's default scope set (broader than only entry read/write).
    // `lookup_or_create` still requires read and write behavior because it reads to check existence
    // and writes when creating.
    let private_key_pem = tokio::fs::read_to_string(jwt_key_path("signing_private_key.pem"))
        .await
        .expect("read JWT private key");
    let signing_key = EncodingKey::from_ec_pem(private_key_pem.as_bytes()).expect("EncodingKey");
    let token = make_token(&signing_key, &jwt_issuer, &jwt_audience, &index_name, None);

    let client = UdexClient::connect(
        ClientOptions::builder()
            .endpoint(format!("https://{SDK_JWT_BIND_ADDR}"))
            .ca_cert_pem_bytes(ca_pem)
            .static_bearer_token(token)
            .build()
            .unwrap(),
    )
    .await
    .expect("SDK connect failed");

    (client, index_name)
}

pub async fn data(serial: bool) -> Data<'static, JwtFixture> {
    static DATA: OnceLock<MaybeOnceAsync<JwtFixture>> = OnceLock::new();
    DATA.get_or_init(|| MaybeOnceAsync::new(|| Box::pin(init_jwt_fixture())))
        .data(serial)
        .await
}

// ── Hydra fixture ─────────────────────────────────────────────────────────────

type HydraFixture = (
    UdexClient,
    String, // index name
);

async fn init_hydra_fixture() -> HydraFixture {
    udex_server::logging::init_test_tracing();
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let admin_url = hydra_admin_url();
    let public_url = hydra_public_url();
    let issuer = std::env::var("HYDRA_ISSUER")
        .unwrap_or_else(|_| format!("{}/", public_url.trim_end_matches('/')));
    let jwks_url = format!("{public_url}/.well-known/jwks.json");

    let index_name = format!("{ID_HYDRA_PREFIX}-index");
    let audience = format!("{ID_HYDRA_PREFIX}-audience");
    let client_id = format!("{ID_HYDRA_PREFIX}-client");
    let client_secret = "sdk-hydra-test-secret".to_string();
    let bind_address: SocketAddr = SDK_HYDRA_BIND_ADDR.parse().unwrap();

    let datastore_fixtures = init_postgres().await;
    let datastore = datastore_fixtures.0;

    let server_config = udex_server::config::ServerConfig {
        bind_address,
        request_timeout: std::time::Duration::from_secs(30),
        max_connections: 1000,
        max_message_size: 4 * 1024 * 1024,
        tls: udex_server::config::TlsConfig {
            cert: bind_file_secret(&server_cert_path("server.crt")),
            key: bind_file_secret(&server_cert_path("server.key")),
        },
        observability: None,
        init_indexes: vec![CreateIndexRequest {
            name: index_name.clone(),
            display_name: index_name.clone(),
            description: index_name.clone(),
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
            jwt_audience: Some(audience.clone()),
            danger_allow_non_tls: true,
            scope_claim_name: Some("scp".to_string()),
            mask_subject_in_logs: false,
            jwks_max_failed_refreshes: None,
            jwks_backoff_factor_secs: None,
            jwks_max_age_secs: None,
        },
    };

    let ca_pem = tokio::fs::read(server_cert_path("ca.crt"))
        .await
        .expect("read CA cert");

    tokio::spawn(async move {
        udex_server::server::serve(server_config, datastore)
            .await
            .expect("hydra server failed");
    });

    wait_for_server(SDK_HYDRA_BIND_ADDR, &ca_pem).await;

    // Register a Hydra client with all scopes needed by the integration tests.
    // udex:index:v1:create   — required by create_index
    // udex:index:v1:*:delete — required by delete_index (glob matches any index name)
    register_hydra_client(
        &admin_url,
        &client_id,
        &client_secret,
        &audience,
        &format!(
            "udex:index:v1:list \
             udex:index:v1:create \
             udex:index:v1:{index_name}:read \
             udex:index:v1:*:delete \
             udex:entry:v1:{index_name}:create \
             udex:entry:v1:{index_name}:read \
             udex:entry:v1:{index_name}:write \
             udex:entry:v1:{index_name}:delete"
        ),
    )
    .await;

    let token_url = format!("{public_url}/oauth2/token");

    let client = UdexClient::connect(
        ClientOptions::builder()
            .endpoint(format!("https://{SDK_HYDRA_BIND_ADDR}"))
            .ca_cert_pem_bytes(ca_pem)
            .client_credentials(token_url, &client_id, &client_secret)
            .audience(&audience)
            .scope(format!(
                "udex:index:v1:list \
                 udex:index:v1:create \
                 udex:index:v1:{index_name}:read \
                 udex:index:v1:*:delete \
                 udex:entry:v1:{index_name}:create \
                 udex:entry:v1:{index_name}:read \
                 udex:entry:v1:{index_name}:write \
                 udex:entry:v1:{index_name}:delete"
            ))
            .danger_allow_non_tls() // Hydra token endpoint is plain HTTP in the dev environment
            .build()
            .unwrap(),
    )
    .await
    .expect("SDK hydra connect failed");

    (client, index_name)
}

pub async fn data_hydra(serial: bool) -> Data<'static, HydraFixture> {
    static HYDRA_DATA: OnceLock<MaybeOnceAsync<HydraFixture>> = OnceLock::new();
    HYDRA_DATA
        .get_or_init(|| MaybeOnceAsync::new(|| Box::pin(init_hydra_fixture())))
        .data(serial)
        .await
}

// ── Test helpers ──────────────────────────────────────────────────────────────

/// Signs a short-lived JWT with the given signing key.
fn make_token(
    signing_key: &EncodingKey,
    issuer: &str,
    audience: &str,
    index_name: &str,
    scope_override: Option<&str>,
) -> String {
    let now = OffsetDateTime::now_utc().unix_timestamp() as usize;
    let default_scope;
    let scope = if let Some(s) = scope_override {
        s
    } else {
        default_scope = format!(
            "udex:index:v1:list \
             udex:index:v1:create \
             udex:index:v1:{index_name}:read \
             udex:index:v1:*:delete \
             udex:entry:v1:{index_name}:create \
             udex:entry:v1:{index_name}:read \
             udex:entry:v1:{index_name}:write \
             udex:entry:v1:{index_name}:delete"
        );
        &default_scope
    };
    let claims = udex_api::authz::claims::Claims::new(
        "sdk-test-subject".to_string(),
        issuer.to_string(),
        audience.to_string(),
        now + 3600,
        now,
    )
    .with_scope(scope.to_string());
    encode(
        &Header::new(jsonwebtoken::Algorithm::ES256),
        &claims,
        signing_key,
    )
    .expect("JWT encode")
}

fn context_input(pairs: &[(&str, &str)]) -> ContextInput {
    ContextInput {
        pairs: pairs
            .iter()
            .map(|(k, v)| KeyValuePair {
                key: k.to_string(),
                value: Some(Value {
                    value: Some(udex_sdk::value::Value::StringValue(v.to_string())),
                }),
                kek_id: None,
                dek: None,
            })
            .collect(),
    }
}

// ── JWT-backed tests (always run) ─────────────────────────────────────────────

#[rstest]
#[tokio_shared_rt::test]
async fn test_sdk_health_serving() {
    let d = data(false).await;
    let client = &d.0;

    let status = client.health().await.expect("health() failed");
    assert_eq!(status, HealthStatus::Serving);
}

#[rstest]
#[tokio_shared_rt::test]
async fn test_sdk_connect_and_list_indices() {
    let d = data(false).await;
    let client = &d.0;
    let index_name = &d.1;

    let indices = client.list_indices().await.expect("list_indices failed");
    assert!(
        indices.iter().any(|i| &i.name == index_name),
        "test index not found in list"
    );
}

#[rstest]
#[tokio_shared_rt::test]
async fn test_sdk_describe_index() {
    let d = data(false).await;
    let client = &d.0;
    let index_name = &d.1;

    let index = client
        .describe_index(index_name)
        .await
        .expect("describe_index failed");
    assert_eq!(&index.name, index_name);
}

#[rstest]
#[tokio_shared_rt::test]
async fn test_sdk_create_and_lookup_entry() {
    let d = data(false).await;
    let client = &d.0;
    let index_name = &d.1;

    let ctx = context_input(&[("sdk_test_key", "sdk_test_value_create_lookup")]);

    let created = client
        .create_entry(index_name, ctx.clone())
        .await
        .expect("create_entry failed");
    assert!(!created.key.is_empty());
    assert!(!created.context_hash.is_empty());

    // Lookup context by key.
    let found_ctx = client
        .lookup_context_by_key(index_name, &created.key)
        .await
        .expect("lookup_context_by_key failed");
    assert_eq!(found_ctx.pairs[0].key, "sdk_test_key");

    // Reverse lookup: key by context hash.
    let found_key = client
        .lookup_key_by_context(index_name, &created.context_hash)
        .await
        .expect("lookup_key_by_context failed");
    assert_eq!(found_key.as_deref(), Some(created.key.as_str()));
}

#[rstest]
#[tokio_shared_rt::test]
async fn test_sdk_create_entry_idempotent() {
    let d = data(false).await;
    let client = &d.0;
    let index_name = &d.1;

    let ctx = context_input(&[("sdk_idem_key", "sdk_idem_value")]);

    let first = client
        .create_entry(index_name, ctx.clone())
        .await
        .expect("first create failed");
    let second = client
        .create_entry(index_name, ctx)
        .await
        .expect("second create failed");

    assert_eq!(
        first.key, second.key,
        "idempotent create returned different keys"
    );
    assert_eq!(first.context_hash, second.context_hash);
}

#[rstest]
#[tokio_shared_rt::test]
async fn test_sdk_delete_entry() {
    let d = data(false).await;
    let client = &d.0;
    let index_name = &d.1;

    let ctx = context_input(&[("sdk_del_key", "sdk_del_value")]);
    let created = client
        .create_entry(index_name, ctx)
        .await
        .expect("create for delete failed");

    client
        .delete_entry(index_name, &created.key)
        .await
        .expect("delete_entry failed");

    // After deletion the lookup should return NOT_FOUND.
    let err = client
        .lookup_context_by_key(index_name, &created.key)
        .await
        .unwrap_err();
    assert!(
        matches!(&err, udex_sdk::Error::Rpc(s) if s.code() == udex_sdk::grpc_code::NOT_FOUND),
        "expected NOT_FOUND after delete, got: {err}"
    );
}

#[rstest]
#[tokio_shared_rt::test]
async fn test_sdk_lookup_nonexistent_returns_none() {
    let d = data(false).await;
    let client = &d.0;
    let index_name = &d.1;

    let key = client
        .lookup_key_by_context(index_name, "nonexistent-hash-xyz-123")
        .await
        .expect("lookup_key_by_context failed");
    assert!(key.is_none(), "expected None for unknown context hash");
}

#[rstest]
#[tokio_shared_rt::test]
async fn test_sdk_bulk_write_and_read() {
    use udex_api::entry::{
        bulk_read_entry_operation_result, bulk_write_entry_operation_result, CreateEntryRequest,
        LookupContextByKeyRequest,
    };
    use udex_sdk::{
        bulk_read_entry_operation, bulk_write_entry_operation, BulkReadEntryOperation,
        BulkWriteEntryOperation,
    };

    let d = data(false).await;
    let client = &d.0;
    let index_name = &d.1;

    let ops: Vec<BulkWriteEntryOperation> = (0..5)
        .map(|i| BulkWriteEntryOperation {
            operation: Some(bulk_write_entry_operation::Operation::CreateEntry(
                CreateEntryRequest {
                    index_name: index_name.clone(),
                    context: Some(context_input(&[(
                        "sdk_bulk_key",
                        &format!("sdk_bulk_value_{i}"),
                    )])),
                },
            )),
        })
        .collect();

    let write_results = client
        .bulk_write(index_name, ops)
        .await
        .expect("bulk_write failed");
    assert_eq!(write_results.len(), 5);

    // Extract the created keys from results.
    let keys: Vec<String> = write_results
        .iter()
        .map(|r| match r.result.as_ref().expect("missing result") {
            bulk_write_entry_operation_result::Result::CreateEntry(c) => c.key.clone(),
            bulk_write_entry_operation_result::Result::DeleteEntry(_) => {
                panic!("unexpected delete")
            }
            bulk_write_entry_operation_result::Result::LookupOrCreate(_) => {
                panic!("unexpected lookup_or_create")
            }
        })
        .collect();

    // Bulk read back by key.
    let read_ops: Vec<BulkReadEntryOperation> = keys
        .iter()
        .map(|key| BulkReadEntryOperation {
            operation: Some(bulk_read_entry_operation::Operation::LookupContext(
                LookupContextByKeyRequest {
                    index_name: index_name.clone(),
                    key: key.clone(),
                },
            )),
        })
        .collect();

    let read_results = client
        .bulk_read(index_name, read_ops)
        .await
        .expect("bulk_read failed");
    assert_eq!(read_results.len(), 5);

    for result in &read_results {
        match result.result.as_ref().expect("missing result") {
            bulk_read_entry_operation_result::Result::LookupContext(r) => {
                assert!(r.context.is_some(), "expected context in bulk read result");
            }
            bulk_read_entry_operation_result::Result::LookupKey(_) => {
                panic!("unexpected LookupKey result")
            }
        }
    }
}

/// `lookup_or_create_entry` — first call for an unseen context creates the entry.
///
/// Exercises the client-side hash computation path: `lookup_or_create_entry`
/// hashes the ContextInput internally before sending the RPC, so the test
/// never needs to supply or compute a hash directly.  The response's
/// `context_hash` must equal what `create_entry` would return for the same
/// pairs, confirming the hash algorithm is stable across SDK methods.
#[rstest]
#[tokio_shared_rt::test]
async fn test_sdk_lookup_or_create_creates_new_entry() {
    let d = data(false).await;
    let client = &d.0;
    let index_name = &d.1;

    let ctx = context_input(&[("loc_sdk_key", "loc_sdk_create_value")]);

    let resp = client
        .lookup_or_create_entry(index_name, ctx.clone())
        .await
        .expect("lookup_or_create_entry failed");

    assert!(resp.created, "created must be true for an unseen context");
    assert!(!resp.key.is_empty(), "key must not be empty");
    assert!(
        !resp.context_hash.is_empty(),
        "context_hash must not be empty"
    );

    // The hash must be consistent with what create_entry returns for the same pairs.
    let via_create = client
        .create_entry(index_name, ctx)
        .await
        .expect("create_entry failed");
    assert_eq!(
        resp.context_hash, via_create.context_hash,
        "lookup_or_create and create_entry must compute the same context hash"
    );
    assert_eq!(
        resp.key, via_create.key,
        "lookup_or_create and create_entry must return the same key for the same context"
    );
}

/// `lookup_or_create_entry` — second call with the same context returns the
/// existing key with created=false.
#[rstest]
#[tokio_shared_rt::test]
async fn test_sdk_lookup_or_create_returns_existing_entry() {
    let d = data(false).await;
    let client = &d.0;
    let index_name = &d.1;

    let ctx = context_input(&[("loc_sdk_key", "loc_sdk_found_value")]);

    let first = client
        .lookup_or_create_entry(index_name, ctx.clone())
        .await
        .expect("first lookup_or_create_entry failed");
    assert!(first.created, "first call must create the entry");

    let second = client
        .lookup_or_create_entry(index_name, ctx)
        .await
        .expect("second lookup_or_create_entry failed");

    assert!(!second.created, "second call must not create a new entry");
    assert_eq!(
        second.key, first.key,
        "second call must return the same key as the first"
    );
    assert_eq!(
        second.context_hash, first.context_hash,
        "context_hash must be stable across calls"
    );
}

#[rstest]
#[tokio_shared_rt::test]
async fn test_sdk_invalid_token_returns_rpc_error() {
    // JWT-specific: exercises static_bearer_token rejection at the server.
    // The OAuth2 equivalent is test_sdk_oauth2_invalid_credentials_return_auth_error,
    // which covers the token-acquisition failure path instead.
    let _d = data(false).await;

    // Connect with a deliberately wrong static token; server should reject it.
    let ca_pem = tokio::fs::read(server_cert_path("ca.crt"))
        .await
        .expect("read CA cert");

    let client = UdexClient::connect(
        ClientOptions::builder()
            .endpoint(format!("https://{SDK_JWT_BIND_ADDR}"))
            .ca_cert_pem_bytes(ca_pem)
            .static_bearer_token("this-is-not-a-valid-jwt")
            .build()
            .unwrap(),
    )
    .await
    .expect("connect should succeed even with a bad token");

    let err = client.list_indices().await.unwrap_err();
    assert!(
        matches!(&err, udex_sdk::Error::Rpc(s)
            if s.code() == udex_sdk::grpc_code::UNAUTHENTICATED || s.code() == udex_sdk::grpc_code::PERMISSION_DENIED),
        "expected Unauthenticated/PermissionDenied, got: {err}"
    );
}

#[rstest]
#[tokio_shared_rt::test]
async fn test_sdk_envelope_encrypted_entry() {
    use aes_gcm::{
        aead::{Aead, AeadCore, KeyInit, OsRng},
        Aes256Gcm, Key, Nonce,
    };
    use base64::{engine::general_purpose::STANDARD as B64, Engine as _};

    let d = data(false).await;
    let client = &d.0;
    let index_name = &d.1;

    // The KEK id is an opaque label the client uses to identify which key was
    // used to wrap the DEK; the server stores and echoes it back verbatim.
    let kek_id = "test-kek-v1";

    // KEK: the client's master key (in production, stored in a key vault).
    let kek_bytes = Aes256Gcm::generate_key(OsRng);
    let kek = Aes256Gcm::new(&kek_bytes);

    // DEK: a fresh data-encryption key scoped to this context.
    let dek_bytes = Aes256Gcm::generate_key(OsRng);
    let dek = Aes256Gcm::new(&dek_bytes);

    // Encrypt the sensitive value with the DEK.
    // Wire format: nonce (12 B) || ciphertext, base64-encoded.
    let plaintext_email = "alice@example.com";
    let value_nonce = Aes256Gcm::generate_nonce(OsRng);
    let value_ct = dek
        .encrypt(&value_nonce, plaintext_email.as_bytes())
        .expect("encrypt value");
    let encrypted_value = B64.encode([value_nonce.as_slice(), &value_ct].concat());

    // Wrap the DEK with the KEK.
    let dek_nonce = Aes256Gcm::generate_nonce(OsRng);
    let dek_ct = kek
        .encrypt(&dek_nonce, dek_bytes.as_slice())
        .expect("encrypt DEK");
    let encrypted_dek = B64.encode([dek_nonce.as_slice(), &dek_ct].concat());

    // Build a context: one plaintext pair and one encrypted pair.
    // The encrypted pair carries kek_id and the wrapped DEK directly on the pair.
    let ctx = ContextInput {
        pairs: vec![
            KeyValuePair {
                key: "user_id".to_string(),
                value: Some(Value {
                    value: Some(udex_sdk::value::Value::StringValue("42".to_string())),
                }),
                kek_id: None,
                dek: None,
            },
            KeyValuePair {
                key: "email".to_string(),
                value: Some(Value {
                    value: Some(udex_sdk::value::Value::StringValue(encrypted_value)),
                }),
                kek_id: Some(kek_id.to_string()),
                dek: Some(encrypted_dek),
            },
        ],
    };

    let created = client
        .create_entry(index_name, ctx)
        .await
        .expect("create_entry failed");
    assert!(!created.key.is_empty());

    // Retrieve the stored context.
    let found_ctx = client
        .lookup_context_by_key(index_name, &created.key)
        .await
        .expect("lookup_context_by_key failed");

    // Find the encrypted pair; verify kek_id and dek were echoed back unchanged.
    let email_pair = found_ctx
        .pairs
        .iter()
        .find(|p| p.key == "email")
        .expect("email pair missing");
    assert_eq!(email_pair.kek_id.as_deref(), Some(kek_id));
    let returned_encrypted_dek = email_pair.dek.as_deref().expect("missing dek on pair");

    // Unwrap the DEK using the KEK.
    let dek_bytes_enc = B64
        .decode(returned_encrypted_dek)
        .expect("base64 decode DEK");
    let (dek_nonce_bytes, dek_ct_bytes) = dek_bytes_enc.split_at(12);
    let unwrapped_dek = kek
        .decrypt(Nonce::from_slice(dek_nonce_bytes), dek_ct_bytes)
        .expect("decrypt DEK");
    let dek_dec = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&unwrapped_dek));

    // Decrypt the value.
    let enc_value = match email_pair
        .value
        .as_ref()
        .and_then(|v| v.value.as_ref())
        .expect("missing value")
    {
        udex_sdk::value::Value::StringValue(s) => s.clone(),
        v => panic!("unexpected value type: {v:?}"),
    };
    let enc_bytes = B64.decode(&enc_value).expect("base64 decode value");
    let (val_nonce_bytes, val_ct_bytes) = enc_bytes.split_at(12);
    let decrypted = dek_dec
        .decrypt(Nonce::from_slice(val_nonce_bytes), val_ct_bytes)
        .expect("decrypt value");

    assert_eq!(
        String::from_utf8(decrypted).expect("UTF-8"),
        plaintext_email
    );
}

// ── Hydra-backed tests ────────────────────────────────────────────────────────
//
// These tests mirror the JWT-fixture tests above but drive the full OAuth2
// client-credentials token lifecycle through Hydra, which is always available
// in the compose development environment.

#[rstest]
#[tokio_shared_rt::test]
async fn test_sdk_oauth2_health_serving() {
    let d = data_hydra(false).await;
    let client = &d.0;

    let status = client.health().await.expect("health() failed");
    assert_eq!(status, HealthStatus::Serving);
}

#[rstest]
#[tokio_shared_rt::test]
async fn test_sdk_oauth2_list_and_describe_index() {
    let d = data_hydra(false).await;
    let client = &d.0;
    let index_name = &d.1;

    let indices = client
        .list_indices()
        .await
        .expect("list_indices via Hydra failed");
    assert!(indices.iter().any(|i| &i.name == index_name));

    let index = client
        .describe_index(index_name)
        .await
        .expect("describe_index via Hydra failed");
    assert_eq!(&index.name, index_name);
}

#[rstest]
#[tokio_shared_rt::test]
async fn test_sdk_oauth2_create_and_lookup_entry() {
    let d = data_hydra(false).await;
    let client = &d.0;
    let index_name = &d.1;

    let ctx = context_input(&[("hydra_key", "hydra_value_create_lookup")]);

    let created = client
        .create_entry(index_name, ctx)
        .await
        .expect("create_entry via Hydra failed");
    assert!(!created.key.is_empty());

    let found_ctx = client
        .lookup_context_by_key(index_name, &created.key)
        .await
        .expect("lookup_context_by_key via Hydra failed");
    assert_eq!(found_ctx.pairs[0].key, "hydra_key");

    let found_key = client
        .lookup_key_by_context(index_name, &created.context_hash)
        .await
        .expect("lookup_key_by_context via Hydra failed");
    assert_eq!(found_key.as_deref(), Some(created.key.as_str()));
}

#[rstest]
#[tokio_shared_rt::test]
async fn test_sdk_oauth2_create_entry_idempotent() {
    let d = data_hydra(false).await;
    let client = &d.0;
    let index_name = &d.1;

    let ctx = context_input(&[("hydra_idem_key", "hydra_idem_value")]);

    let first = client
        .create_entry(index_name, ctx.clone())
        .await
        .expect("first create via Hydra failed");
    let second = client
        .create_entry(index_name, ctx)
        .await
        .expect("second create via Hydra failed");

    assert_eq!(first.key, second.key);
}

#[rstest]
#[tokio_shared_rt::test]
async fn test_sdk_oauth2_delete_entry() {
    let d = data_hydra(false).await;
    let client = &d.0;
    let index_name = &d.1;

    let ctx = context_input(&[("hydra_del_key", "hydra_del_value")]);
    let created = client
        .create_entry(index_name, ctx)
        .await
        .expect("create for delete via Hydra failed");

    client
        .delete_entry(index_name, &created.key)
        .await
        .expect("delete_entry via Hydra failed");

    let err = client
        .lookup_context_by_key(index_name, &created.key)
        .await
        .unwrap_err();
    assert!(
        matches!(&err, udex_sdk::Error::Rpc(s) if s.code() == udex_sdk::grpc_code::NOT_FOUND),
        "expected NOT_FOUND after delete via Hydra, got: {err}"
    );
}

#[rstest]
#[tokio_shared_rt::test]
async fn test_sdk_oauth2_lookup_nonexistent_returns_none() {
    let d = data_hydra(false).await;
    let client = &d.0;
    let index_name = &d.1;

    let key = client
        .lookup_key_by_context(index_name, "nonexistent-hydra-hash-xyz")
        .await
        .expect("lookup_key_by_context via Hydra failed");
    assert!(key.is_none());
}

#[rstest]
#[tokio_shared_rt::test]
async fn test_sdk_oauth2_bulk_write_and_read() {
    use udex_api::entry::{
        bulk_read_entry_operation_result, bulk_write_entry_operation_result, CreateEntryRequest,
        LookupContextByKeyRequest,
    };
    use udex_sdk::{
        bulk_read_entry_operation, bulk_write_entry_operation, BulkReadEntryOperation,
        BulkWriteEntryOperation,
    };

    let d = data_hydra(false).await;
    let client = &d.0;
    let index_name = &d.1;

    let ops: Vec<BulkWriteEntryOperation> = (0..5)
        .map(|i| BulkWriteEntryOperation {
            operation: Some(bulk_write_entry_operation::Operation::CreateEntry(
                CreateEntryRequest {
                    index_name: index_name.clone(),
                    context: Some(context_input(&[(
                        "hydra_bulk_key",
                        &format!("hydra_bulk_value_{i}"),
                    )])),
                },
            )),
        })
        .collect();

    let write_results = client
        .bulk_write(index_name, ops)
        .await
        .expect("bulk_write via Hydra failed");
    assert_eq!(write_results.len(), 5);

    let keys: Vec<String> = write_results
        .iter()
        .map(|r| match r.result.as_ref().expect("missing result") {
            bulk_write_entry_operation_result::Result::CreateEntry(c) => c.key.clone(),
            bulk_write_entry_operation_result::Result::DeleteEntry(_) => {
                panic!("unexpected delete")
            }
            bulk_write_entry_operation_result::Result::LookupOrCreate(_) => {
                panic!("unexpected lookup_or_create")
            }
        })
        .collect();

    let read_ops: Vec<BulkReadEntryOperation> = keys
        .iter()
        .map(|key| BulkReadEntryOperation {
            operation: Some(bulk_read_entry_operation::Operation::LookupContext(
                LookupContextByKeyRequest {
                    index_name: index_name.clone(),
                    key: key.clone(),
                },
            )),
        })
        .collect();

    let read_results = client
        .bulk_read(index_name, read_ops)
        .await
        .expect("bulk_read via Hydra failed");
    assert_eq!(read_results.len(), 5);

    for result in &read_results {
        match result.result.as_ref().expect("missing result") {
            bulk_read_entry_operation_result::Result::LookupContext(r) => {
                assert!(r.context.is_some());
            }
            bulk_read_entry_operation_result::Result::LookupKey(_) => {
                panic!("unexpected LookupKey result")
            }
        }
    }
}

#[rstest]
#[tokio_shared_rt::test]
async fn test_sdk_oauth2_invalid_credentials_return_auth_error() {
    // Ensure the Hydra-backed server is running.
    let _d = data_hydra(false).await;

    let public_url = hydra_public_url();
    let token_url = format!("{public_url}/oauth2/token");

    let ca_pem = tokio::fs::read(server_cert_path("ca.crt"))
        .await
        .expect("read CA cert");

    let err = UdexClient::connect(
        ClientOptions::builder()
            .endpoint(format!("https://{SDK_HYDRA_BIND_ADDR}"))
            .ca_cert_pem_bytes(ca_pem)
            .client_credentials(&token_url, "nonexistent-client-id", "wrong-secret")
            .audience("wrong-audience")
            .danger_allow_non_tls() // Hydra token endpoint is plain HTTP in the dev environment
            .build()
            .unwrap(),
    )
    .await
    .unwrap_err();

    assert!(
        matches!(err, udex_sdk::Error::Auth(_)),
        "expected Error::Auth for bad credentials, got: {err}"
    );
}

#[rstest]
#[tokio_shared_rt::test]
async fn test_sdk_oauth2_lookup_or_create_creates_new_entry() {
    let d = data_hydra(false).await;
    let client = &d.0;
    let index_name = &d.1;

    let ctx = context_input(&[("hydra_loc_key", "hydra_loc_create_value")]);

    let resp = client
        .lookup_or_create_entry(index_name, ctx.clone())
        .await
        .expect("lookup_or_create_entry via Hydra failed");

    assert!(resp.created, "created must be true for an unseen context");
    assert!(!resp.key.is_empty());
    assert!(!resp.context_hash.is_empty());

    let via_create = client
        .create_entry(index_name, ctx)
        .await
        .expect("create_entry via Hydra failed");
    assert_eq!(resp.context_hash, via_create.context_hash);
    assert_eq!(resp.key, via_create.key);
}

#[rstest]
#[tokio_shared_rt::test]
async fn test_sdk_oauth2_lookup_or_create_returns_existing_entry() {
    let d = data_hydra(false).await;
    let client = &d.0;
    let index_name = &d.1;

    let ctx = context_input(&[("hydra_loc_key", "hydra_loc_found_value")]);

    let first = client
        .lookup_or_create_entry(index_name, ctx.clone())
        .await
        .expect("first lookup_or_create_entry via Hydra failed");
    assert!(first.created);

    let second = client
        .lookup_or_create_entry(index_name, ctx)
        .await
        .expect("second lookup_or_create_entry via Hydra failed");

    assert!(!second.created, "second call must not create a new entry");
    assert_eq!(second.key, first.key);
    assert_eq!(second.context_hash, first.context_hash);
}

#[rstest]
#[tokio_shared_rt::test]
async fn test_sdk_oauth2_envelope_encrypted_entry() {
    use aes_gcm::{
        aead::{Aead, AeadCore, KeyInit, OsRng},
        Aes256Gcm, Key, Nonce,
    };
    use base64::{engine::general_purpose::STANDARD as B64, Engine as _};

    let d = data_hydra(false).await;
    let client = &d.0;
    let index_name = &d.1;

    let kek_id = "hydra-test-kek-v1";
    let kek_bytes = Aes256Gcm::generate_key(OsRng);
    let kek = Aes256Gcm::new(&kek_bytes);
    let dek_bytes = Aes256Gcm::generate_key(OsRng);
    let dek = Aes256Gcm::new(&dek_bytes);

    let plaintext_email = "bob@example.com";
    let value_nonce = Aes256Gcm::generate_nonce(OsRng);
    let value_ct = dek
        .encrypt(&value_nonce, plaintext_email.as_bytes())
        .expect("encrypt value");
    let encrypted_value = B64.encode([value_nonce.as_slice(), &value_ct].concat());

    let dek_nonce = Aes256Gcm::generate_nonce(OsRng);
    let dek_ct = kek
        .encrypt(&dek_nonce, dek_bytes.as_slice())
        .expect("encrypt DEK");
    let encrypted_dek = B64.encode([dek_nonce.as_slice(), &dek_ct].concat());

    let ctx = ContextInput {
        pairs: vec![
            KeyValuePair {
                key: "user_id".to_string(),
                value: Some(Value {
                    value: Some(udex_sdk::value::Value::StringValue("99".to_string())),
                }),
                kek_id: None,
                dek: None,
            },
            KeyValuePair {
                key: "email".to_string(),
                value: Some(Value {
                    value: Some(udex_sdk::value::Value::StringValue(encrypted_value)),
                }),
                kek_id: Some(kek_id.to_string()),
                dek: Some(encrypted_dek),
            },
        ],
    };

    let created = client
        .create_entry(index_name, ctx)
        .await
        .expect("create_entry via Hydra failed");
    assert!(!created.key.is_empty());

    let found_ctx = client
        .lookup_context_by_key(index_name, &created.key)
        .await
        .expect("lookup_context_by_key via Hydra failed");

    let email_pair = found_ctx
        .pairs
        .iter()
        .find(|p| p.key == "email")
        .expect("email pair missing");
    assert_eq!(email_pair.kek_id.as_deref(), Some(kek_id));
    let returned_encrypted_dek = email_pair.dek.as_deref().expect("missing dek on pair");

    let dek_bytes_enc = B64
        .decode(returned_encrypted_dek)
        .expect("base64 decode DEK");
    let (dek_nonce_bytes, dek_ct_bytes) = dek_bytes_enc.split_at(12);
    let unwrapped_dek = kek
        .decrypt(Nonce::from_slice(dek_nonce_bytes), dek_ct_bytes)
        .expect("decrypt DEK");
    let dek_dec = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&unwrapped_dek));

    let enc_value = match email_pair
        .value
        .as_ref()
        .and_then(|v| v.value.as_ref())
        .expect("missing value")
    {
        udex_sdk::value::Value::StringValue(s) => s.clone(),
        v => panic!("unexpected value type: {v:?}"),
    };
    let enc_bytes = B64.decode(&enc_value).expect("base64 decode value");
    let (val_nonce_bytes, val_ct_bytes) = enc_bytes.split_at(12);
    let decrypted = dek_dec
        .decrypt(Nonce::from_slice(val_nonce_bytes), val_ct_bytes)
        .expect("decrypt value");

    assert_eq!(
        String::from_utf8(decrypted).expect("UTF-8"),
        plaintext_email
    );
}

#[rstest]
#[tokio_shared_rt::test]
async fn test_sdk_delete_index_empty() {
    let d = data(false).await;
    let client = &d.0;

    let name = format!("{ID_PREFIX}-delete-empty");

    client
        .create_index(CreateIndexRequest {
            name: name.clone(),
            display_name: "Delete Empty".to_string(),
            description: "delete test".to_string(),
            max_bulk_operations: 100,
            max_key_length: 256,
            max_value_length: 1024,
            max_kv_pairs_per_context: 10,
            hash_algorithm: HashAlgorithm::Xxh3 as i32,
        })
        .await
        .expect("create_index failed");

    client
        .delete_index(&name)
        .await
        .expect("delete_index on empty index should succeed");

    // A second delete must return NOT_FOUND, confirming the index is gone.
    let err = client.delete_index(&name).await.unwrap_err();
    assert!(
        matches!(&err, udex_sdk::Error::Rpc(s) if s.code() == udex_sdk::grpc_code::NOT_FOUND),
        "deleted index should not be found on re-delete, got: {err}"
    );
}

#[rstest]
#[tokio_shared_rt::test]
async fn test_sdk_delete_index_not_empty() {
    let d = data(false).await;
    let client = &d.0;
    let index_name = &d.1;

    // The shared index has entries from other tests; attempting to delete it must fail.
    let err = client.delete_index(index_name).await.unwrap_err();
    assert!(
        matches!(&err, udex_sdk::Error::Rpc(s) if s.code() == udex_sdk::grpc_code::FAILED_PRECONDITION),
        "expected FailedPrecondition for non-empty index, got: {err}"
    );
}

#[rstest]
#[tokio_shared_rt::test]
async fn test_sdk_delete_index_not_found() {
    let d = data(false).await;
    let client = &d.0;

    let err = client
        .delete_index("nonexistent-index-xyz")
        .await
        .unwrap_err();
    assert!(
        matches!(&err, udex_sdk::Error::Rpc(s) if s.code() == udex_sdk::grpc_code::NOT_FOUND),
        "expected NotFound for nonexistent index, got: {err}"
    );
}

#[rstest]
#[tokio_shared_rt::test]
async fn test_sdk_oauth2_delete_index_empty() {
    let d = data_hydra(false).await;
    let client = &d.0;

    let name = format!("{ID_HYDRA_PREFIX}-delete-empty");

    client
        .create_index(CreateIndexRequest {
            name: name.clone(),
            display_name: "Delete Empty Hydra".to_string(),
            description: "hydra delete test".to_string(),
            max_bulk_operations: 100,
            max_key_length: 256,
            max_value_length: 1024,
            max_kv_pairs_per_context: 10,
            hash_algorithm: HashAlgorithm::Xxh3 as i32,
        })
        .await
        .expect("create_index via Hydra failed");

    client
        .delete_index(&name)
        .await
        .expect("delete_index on empty index via Hydra should succeed");

    let err = client.delete_index(&name).await.unwrap_err();
    assert!(
        matches!(&err, udex_sdk::Error::Rpc(s) if s.code() == udex_sdk::grpc_code::NOT_FOUND),
        "deleted index should not be found on re-delete via Hydra, got: {err}"
    );
}

#[rstest]
#[tokio_shared_rt::test]
async fn test_sdk_oauth2_delete_index_not_empty() {
    let d = data_hydra(false).await;
    let client = &d.0;
    let index_name = &d.1;

    // The shared Hydra index has entries from other tests; deletion must fail.
    let err = client.delete_index(index_name).await.unwrap_err();
    assert!(
        matches!(&err, udex_sdk::Error::Rpc(s) if s.code() == udex_sdk::grpc_code::FAILED_PRECONDITION),
        "expected FailedPrecondition for non-empty index via Hydra, got: {err}"
    );
}

#[rstest]
#[tokio_shared_rt::test]
async fn test_sdk_oauth2_delete_index_not_found() {
    let d = data_hydra(false).await;
    let client = &d.0;

    let err = client
        .delete_index("nonexistent-hydra-index-xyz")
        .await
        .unwrap_err();
    assert!(
        matches!(&err, udex_sdk::Error::Rpc(s) if s.code() == udex_sdk::grpc_code::NOT_FOUND),
        "expected NotFound for nonexistent index via Hydra, got: {err}"
    );
}

// ── k8s fixture ───────────────────────────────────────────────────────────────
//
// Connects to a live k3d-deployed udex server. Skipped silently when
// K8S_SERVER_URL is not set. Uses the same Hydra instance as data_hydra().

type K8sFixture = (UdexClient, String); // client, index name

// Transforms the local DATABASE_URL so k3d pods can reach Postgres on the
// host Docker network, and swaps the database name to the test-run DB.
fn k8s_database_url(local_url: &str, db_name: &str) -> String {
    let with_host = local_url
        .replace("@localhost:", "@host.k3d.internal:")
        .replace("@127.0.0.1:", "@host.k3d.internal:");
    // Replace the db name — last path segment after the port/host, before any '?'.
    if let Some(at) = with_host.rfind('@') {
        if let Some(slash) = with_host[at..].find('/') {
            let base = &with_host[..at + slash + 1];
            let rest = &with_host[at + slash + 1..];
            if let Some(q) = rest.find('?') {
                format!("{}{}{}", base, db_name, &rest[q..])
            } else {
                format!("{}{}", base, db_name)
            }
        } else {
            format!("{}/{}", with_host, db_name)
        }
    } else {
        panic!("DATABASE_URL has no @ — unexpected format: {local_url}");
    }
}

// Polls the k8s server healthz endpoint after a helm rollout.
// The readiness probe (tcpSocket) passes as soon as the port is open; Traefik
// IngressRoute routing updates lag slightly behind. Polling gRPC healthz
// directly ensures the new pod is actually serving before create_index runs.
async fn wait_for_k8s_server(server_url: &str, ca_pem: &[u8]) {
    let addr = server_url
        .strip_prefix("https://")
        .expect("K8S_SERVER_URL must start with https://");
    let tls = ClientTlsConfig::new()
        .ca_certificate(Certificate::from_pem(ca_pem))
        .domain_name("host.docker.internal");
    for _ in 0..60 {
        sleep(Duration::from_millis(500)).await;
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
    panic!("k8s server at {server_url} did not become ready within 30s after helm rollout");
}

// Redeploys the k8s server via helm + kubectl with a new DATABASE_URL, then
// waits until the new pod is the only pod and is actually serving gRPC.
// Uses --set-file for the TLS material to avoid shell-escaping issues with
// multiline PEM values.
async fn redeploy_k8s_server(k8s_db_url: &str, server_url: &str, ca_pem: &[u8]) {
    let chart_dir = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../projects/k8s/helm/udex"
    );
    let cert_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../server/tests/certs/server.crt"
    );
    let key_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../server/tests/certs/server.key"
    );
    // Edge cert/key Traefik presents to clients when it terminates TLS. Required
    // by the chart (kubernetes.io/tls Secret); distinct from the pod cert above.
    let edge_cert_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../projects/k8s/traefik/certs/tls.crt"
    );
    let edge_key_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../projects/k8s/traefik/certs/tls.key"
    );
    // OTLP CA the pods trust when exporting telemetry to the local Collector.
    // Required by the chart when observability.enabled (the dev default).
    let otlp_ca_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../projects/observability/certs/ca.crt"
    );
    let mut db_url_file = tempfile::NamedTempFile::new().expect("tempfile for DATABASE_URL");
    db_url_file
        .write_all(k8s_db_url.as_bytes())
        .expect("write DATABASE_URL temp file");
    db_url_file.flush().expect("flush DATABASE_URL temp file");

    let status = tokio::process::Command::new("helm")
        .args([
            "upgrade",
            "--install",
            "udex",
            chart_dir,
            "--set-file",
            &format!("secrets.databaseUrl={}", db_url_file.path().display()),
            "--set-file",
            &format!("secrets.tlsCrt={cert_path}"),
            "--set-file",
            &format!("secrets.tlsKey={key_path}"),
            "--set-file",
            &format!("secrets.traefikTlsCrt={edge_cert_path}"),
            "--set-file",
            &format!("secrets.traefikTlsKey={edge_key_path}"),
            "--set-file",
            &format!("secrets.otlpCa={otlp_ca_path}"),
        ])
        .status()
        .await
        .expect("helm upgrade: command failed to start");
    assert!(status.success(), "helm upgrade returned non-zero exit code");

    let status = tokio::process::Command::new("kubectl")
        .args([
            "rollout",
            "status",
            "deployment/udex-udex",
            "--timeout=120s",
        ])
        .status()
        .await
        .expect("kubectl rollout status: command failed to start");
    assert!(
        status.success(),
        "kubectl rollout status returned non-zero exit code"
    );

    // kubectl rollout status returns when the new pods are Ready, but old pods
    // from the previous revision (pointing at the previous DATABASE_URL — which
    // a prior test run may even have dropped) can still be Running/Terminating
    // and in the LB rotation. Wait until exactly K8S_REPLICAS pods exist AND all
    // are Ready, so no stale pod is left to serve a request, before polling.
    for _ in 0..120u32 {
        let (total, ready) = udex_pod_counts().await;
        if total == K8S_REPLICAS && ready == K8S_REPLICAS {
            break;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    wait_for_k8s_server(server_url, ca_pem).await;
}

// Returns (total udex pods, fully-Ready udex pods). A pod counts as Ready when
// its STATUS is Running and the READY column is "n/n" (n > 0). Total includes
// Pending/Terminating pods so callers can wait for stale pods to fully clear.
// `kubectl get pods --no-headers` columns are: NAME READY STATUS RESTARTS AGE.
async fn udex_pod_counts() -> (usize, usize) {
    let out = tokio::process::Command::new("kubectl")
        .args([
            "get",
            "pods",
            "-l",
            "app.kubernetes.io/name=udex",
            "--no-headers",
        ])
        .output()
        .await
        .expect("kubectl get pods: command failed to start");
    assert!(
        out.status.success(),
        "kubectl get pods exited with {}: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = stdout.lines().filter(|l| !l.trim().is_empty()).collect();
    let ready = lines
        .iter()
        .filter(|line| {
            let mut cols = line.split_whitespace();
            let ready = cols.nth(1).unwrap_or(""); // READY, e.g. "1/1"
            let status = cols.next().unwrap_or(""); // STATUS, e.g. "Running"
            status == "Running"
                && matches!(ready.split_once('/'), Some((a, b)) if a == b && a != "0")
        })
        .count();
    (lines.len(), ready)
}

// Returns the names of the Running udex pods (one per server replica).
async fn discover_udex_pods() -> Vec<String> {
    let out = tokio::process::Command::new("kubectl")
        .args([
            "get",
            "pods",
            "-l",
            "app.kubernetes.io/name=udex",
            "--field-selector=status.phase=Running",
            "-o",
            "jsonpath={.items[*].metadata.name}",
        ])
        .output()
        .await
        .expect("kubectl get pods: command failed to start");
    assert!(
        out.status.success(),
        "kubectl get pods exited with {}: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

// A live `kubectl port-forward localhost:<local_port> -> pod/<pod>:443`, killed
// on drop so tests never leak forwards. This addresses a single server instance
// directly, bypassing Traefik: the hop terminates against the POD's own cert
// (trusted via the server CA, SNI "localhost" — a SAN on the pod cert), not the
// edge cert used on the load-balanced path. See ST0025.
struct PortForward {
    child: std::process::Child,
    local_port: u16,
}

impl PortForward {
    // Spawns the forward and polls until the local port serves gRPC health.
    async fn start(pod: &str, local_port: u16, ca_pem: &[u8]) -> PortForward {
        let child = std::process::Command::new("kubectl")
            .args([
                "port-forward",
                &format!("pod/{pod}"),
                &format!("{local_port}:443"),
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("spawn kubectl port-forward");
        let pf = PortForward { child, local_port };

        let tls = ClientTlsConfig::new()
            .ca_certificate(Certificate::from_pem(ca_pem))
            .domain_name("localhost");
        for _ in 0..40 {
            sleep(Duration::from_millis(250)).await;
            let Ok(ch) = Channel::from_shared(pf.endpoint())
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
                return pf;
            }
        }
        panic!("port-forward to pod {pod} on :{local_port} did not become ready within 10s");
    }

    fn endpoint(&self) -> String {
        format!("https://localhost:{}", self.local_port)
    }
}

impl Drop for PortForward {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

async fn init_k8s_fixture() -> Option<K8sFixture> {
    let server_url = match std::env::var("K8S_SERVER_URL") {
        Ok(url) => url,
        Err(_) => return None,
    };

    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let admin_url = hydra_admin_url();
    let public_url = hydra_public_url();

    let index_name = format!("{ID_K8S_PREFIX}-index");
    // Audience must match jwtAudience in the k8s Helm values ("udex").
    let audience = "udex".to_string();
    let client_id = format!("{ID_K8S_PREFIX}-client");
    let client_secret = "sdk-k8s-test-secret".to_string();

    // Traefik terminates client TLS at the ingress, so the client trusts the
    // EDGE CA (projects/k8s/traefik/certs), not the pod's CA. See ST0024.
    let ca_pem = tokio::fs::read(edge_cert_path("ca.crt"))
        .await
        .expect("read edge CA cert for k8s fixture");

    // Create a fresh isolated test database — mirrors init_postgres() used by
    // the in-process fixtures. The database is registered for automatic cleanup
    // on test-binary exit (same #[ctor::dtor] mechanism).
    let (_datastore, _pool, db_name) = init_postgres().await;
    let local_db_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for k8s tests");
    let k8s_db_url = k8s_database_url(&local_db_url, &db_name);

    // Redeploy the k8s server against the fresh database.
    redeploy_k8s_server(&k8s_db_url, &server_url, &ca_pem).await;

    register_hydra_client(
        &admin_url,
        &client_id,
        &client_secret,
        &audience,
        &format!(
            "udex:index:v1:list \
             udex:index:v1:create \
             udex:index:v1:{index_name}:read \
             udex:index:v1:*:delete \
             udex:entry:v1:{index_name}:create \
             udex:entry:v1:{index_name}:read \
             udex:entry:v1:{index_name}:write \
             udex:entry:v1:{index_name}:delete"
        ),
    )
    .await;

    let token_url = format!("{public_url}/oauth2/token");

    let client = UdexClient::connect(
        ClientOptions::builder()
            .endpoint(&server_url)
            .ca_cert_pem_bytes(ca_pem)
            .client_credentials(token_url, &client_id, &client_secret)
            .audience(&audience)
            .scope(format!(
                "udex:index:v1:list \
                 udex:index:v1:create \
                 udex:index:v1:{index_name}:read \
                 udex:index:v1:*:delete \
                 udex:entry:v1:{index_name}:create \
                 udex:entry:v1:{index_name}:read \
                 udex:entry:v1:{index_name}:write \
                 udex:entry:v1:{index_name}:delete"
            ))
            .danger_allow_non_tls() // Hydra token endpoint is plain HTTP in the dev environment
            .build()
            .unwrap(),
    )
    .await
    .expect("SDK k8s connect failed");

    // Fresh database — create the test index directly, no idempotency needed.
    client
        .create_index(CreateIndexRequest {
            name: index_name.clone(),
            display_name: index_name.clone(),
            description: "k8s integration test index".to_string(),
            max_bulk_operations: 100,
            max_key_length: 256,
            max_value_length: 1024,
            max_kv_pairs_per_context: 50,
            hash_algorithm: HashAlgorithm::Xxh3 as i32,
        })
        .await
        .expect("k8s fixture: failed to create test index");

    Some((client, index_name))
}

pub async fn data_k8s() -> Option<K8sFixture> {
    static K8S_DATA: tokio::sync::OnceCell<Option<K8sFixture>> = tokio::sync::OnceCell::const_new();
    K8S_DATA.get_or_init(init_k8s_fixture).await.clone()
}

// ── k8s-backed tests ──────────────────────────────────────────────────────────
//
// All tests return early (silent skip) when K8S_SERVER_URL is not set.
// Run against a live cluster: bash projects/k8s/scripts/cluster-create.sh &&
// bash projects/k8s/scripts/image-build.sh && bash projects/k8s/scripts/image-load.sh &&
// bash projects/k8s/scripts/deploy.sh

#[rstest]
#[tokio_shared_rt::test]
async fn test_sdk_k8s_list_indices() {
    let fixture = data_k8s().await;
    let Some((client, index_name)) = fixture.as_ref() else {
        return;
    };

    let indices = client
        .list_indices()
        .await
        .expect("list_indices against k8s failed");
    assert!(
        indices.iter().any(|i| &i.name == index_name),
        "k8s test index not found in list"
    );
}

#[rstest]
#[tokio_shared_rt::test]
async fn test_sdk_k8s_describe_index() {
    let fixture = data_k8s().await;
    let Some((client, index_name)) = fixture.as_ref() else {
        return;
    };

    let index = client
        .describe_index(index_name)
        .await
        .expect("describe_index against k8s failed");
    assert_eq!(&index.name, index_name);
}

#[rstest]
#[tokio_shared_rt::test]
async fn test_sdk_k8s_create_and_lookup_entry() {
    let fixture = data_k8s().await;
    let Some((client, index_name)) = fixture.as_ref() else {
        return;
    };

    let ctx = context_input(&[("k8s_key", "k8s_value_create_lookup")]);

    let created = client
        .create_entry(index_name, ctx.clone())
        .await
        .expect("create_entry against k8s failed");
    assert!(!created.key.is_empty());

    let found_ctx = client
        .lookup_context_by_key(index_name, &created.key)
        .await
        .expect("lookup_context_by_key against k8s failed");
    assert_eq!(found_ctx.pairs[0].key, "k8s_key");

    let found_key = client
        .lookup_key_by_context(index_name, &created.context_hash)
        .await
        .expect("lookup_key_by_context against k8s failed");
    assert_eq!(found_key.as_deref(), Some(created.key.as_str()));
}

#[rstest]
#[tokio_shared_rt::test]
async fn test_sdk_k8s_create_entry_idempotent() {
    let fixture = data_k8s().await;
    let Some((client, index_name)) = fixture.as_ref() else {
        return;
    };

    let ctx = context_input(&[("k8s_idem_key", "k8s_idem_value")]);

    let first = client
        .create_entry(index_name, ctx.clone())
        .await
        .expect("first k8s create failed");
    let second = client
        .create_entry(index_name, ctx)
        .await
        .expect("second k8s create failed");

    assert_eq!(
        first.key, second.key,
        "idempotent create returned different keys"
    );
    assert_eq!(first.context_hash, second.context_hash);
}

#[rstest]
#[tokio_shared_rt::test]
async fn test_sdk_k8s_delete_entry() {
    let fixture = data_k8s().await;
    let Some((client, index_name)) = fixture.as_ref() else {
        return;
    };

    let ctx = context_input(&[("k8s_del_key", "k8s_del_value")]);
    let created = client
        .create_entry(index_name, ctx)
        .await
        .expect("k8s create for delete failed");

    client
        .delete_entry(index_name, &created.key)
        .await
        .expect("k8s delete_entry failed");

    let err = client
        .lookup_context_by_key(index_name, &created.key)
        .await
        .unwrap_err();
    assert!(
        matches!(&err, udex_sdk::Error::Rpc(s) if s.code() == udex_sdk::grpc_code::NOT_FOUND),
        "expected NOT_FOUND after k8s delete, got: {err}"
    );
}

#[rstest]
#[tokio_shared_rt::test]
async fn test_sdk_k8s_lookup_or_create_entry() {
    let fixture = data_k8s().await;
    let Some((client, index_name)) = fixture.as_ref() else {
        return;
    };

    let ctx = context_input(&[("k8s_loc_key", "k8s_loc_value")]);

    let first = client
        .lookup_or_create_entry(index_name, ctx.clone())
        .await
        .expect("k8s lookup_or_create_entry failed");
    assert!(!first.key.is_empty());

    let second = client
        .lookup_or_create_entry(index_name, ctx)
        .await
        .expect("k8s lookup_or_create_entry (second) failed");

    assert_eq!(second.key, first.key);
    assert!(!second.created, "second call must not create a new entry");
}

// ── Multi-instance (cluster) tests ──────────────────────────────────────────────
//
// These verify the server behaves correctly with >1 replica (ST0025). They reuse
// the data_k8s() deployment (2 replicas, shared DB) and address each pod directly
// via `kubectl port-forward`, bypassing the round-robin load balancer so a request
// can be pinned to a specific instance. Direct hops trust the pod cert (server CA,
// SNI "localhost"). Skipped (with the rest of the k8s suite) when K8S_SERVER_URL
// is unset.

// Proof-of-life for the direct-addressing harness: both replicas are discoverable
// and each serves gRPC health when addressed directly (not via the LB).
#[rstest]
#[tokio_shared_rt::test]
async fn test_sdk_k8s_multi_direct_health() {
    // Guarantees the 2-replica deployment is up (and skips when k8s is disabled).
    if data_k8s().await.is_none() {
        return;
    }

    let ca_pem = tokio::fs::read(server_cert_path("ca.crt"))
        .await
        .expect("read server CA for direct pod addressing");

    let pods = discover_udex_pods().await;
    assert_eq!(
        pods.len(),
        K8S_REPLICAS,
        "expected {K8S_REPLICAS} running udex pods, got {pods:?}"
    );

    // Forward each pod to a distinct local port. The guards live until the end of
    // the test, then RAII-drop kills the kubectl port-forward children.
    let mut forwards = Vec::new();
    for (i, pod) in pods.iter().enumerate() {
        let pf = PortForward::start(pod, 18443 + i as u16, &ca_pem).await;
        forwards.push((pod.clone(), pf));
    }

    for (pod, pf) in &forwards {
        let tls = ClientTlsConfig::new()
            .ca_certificate(Certificate::from_pem(ca_pem.clone()))
            .domain_name("localhost");
        let ch = Channel::from_shared(pf.endpoint())
            .unwrap()
            .tls_config(tls)
            .unwrap()
            .connect()
            .await
            .unwrap_or_else(|e| panic!("direct connect to pod {pod} failed: {e}"));
        let status = HealthClient::new(ch)
            .check(HealthCheckRequest {
                service: "".to_string(),
            })
            .await
            .unwrap_or_else(|e| panic!("direct health check on pod {pod} failed: {e}"))
            .into_inner()
            .status;
        assert_eq!(
            status,
            tonic_health::pb::health_check_response::ServingStatus::Serving as i32,
            "pod {pod} not SERVING when addressed directly"
        );
    }
}

// Builds an SDK client pinned to one server instance via its port-forward
// endpoint. Direct hops trust the pod cert (server CA, SNI "localhost"); auth is
// the usual Hydra client_credentials flow (token endpoint is plain HTTP in dev).
async fn build_pod_client(
    endpoint: &str,
    ca_pem: Vec<u8>,
    token_url: &str,
    client_id: &str,
    client_secret: &str,
    audience: &str,
    scope: &str,
) -> UdexClient {
    UdexClient::connect(
        ClientOptions::builder()
            .endpoint(endpoint)
            .ca_cert_pem_bytes(ca_pem)
            .client_credentials(token_url.to_string(), client_id, client_secret)
            .audience(audience)
            .scope(scope.to_string())
            .danger_allow_non_tls()
            .build()
            .unwrap(),
    )
    .await
    .unwrap_or_else(|e| panic!("SDK connect to pod endpoint {endpoint} failed: {e}"))
}

// Cross-instance correctness: with 2 replicas sharing one datastore, a mutation
// made through one instance must be observable through the other. Each request is
// pinned to a specific pod via port-forward (NOT the round-robin LB), so the test
// genuinely exercises instance A vs instance B rather than hitting the same pod
// twice by luck. Proves the server holds no per-instance state that another
// instance can't see (CLAUDE.md: "Server MUST be stateless"). See ST0025.
#[rstest]
#[tokio_shared_rt::test]
async fn test_sdk_k8s_multi_cross_instance_crud() {
    // Guarantees the 2-replica deployment is up (and skips when k8s is disabled).
    if data_k8s().await.is_none() {
        return;
    }

    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let ca_pem = tokio::fs::read(server_cert_path("ca.crt"))
        .await
        .expect("read server CA for cross-instance test");

    let pods = discover_udex_pods().await;
    assert_eq!(
        pods.len(),
        K8S_REPLICAS,
        "expected {K8S_REPLICAS} running udex pods, got {pods:?}"
    );

    // Pin a client to each pod. Ports are distinct from the smoke test's so the
    // two can run concurrently. Forwards drop (and are killed) at end of scope.
    let pf_a = PortForward::start(&pods[0], 18445, &ca_pem).await;
    let pf_b = PortForward::start(&pods[1], 18446, &ca_pem).await;

    // Hydra client + scopes for this test's own index (distinct from data_k8s()).
    let index_name = format!("{ID_K8S_PREFIX}-multi-index");
    let audience = "udex".to_string();
    let client_id = format!("{ID_K8S_PREFIX}-multi-client");
    let client_secret = "sdk-k8s-multi-test-secret".to_string();
    let scope = format!(
        "udex:index:v1:list \
         udex:index:v1:create \
         udex:index:v1:{index_name}:read \
         udex:index:v1:*:delete \
         udex:entry:v1:{index_name}:create \
         udex:entry:v1:{index_name}:read \
         udex:entry:v1:{index_name}:write \
         udex:entry:v1:{index_name}:delete"
    );
    register_hydra_client(
        &hydra_admin_url(),
        &client_id,
        &client_secret,
        &audience,
        &scope,
    )
    .await;
    let token_url = format!("{}/oauth2/token", hydra_public_url());

    let client_a = build_pod_client(
        &pf_a.endpoint(),
        ca_pem.clone(),
        &token_url,
        &client_id,
        &client_secret,
        &audience,
        &scope,
    )
    .await;
    let client_b = build_pod_client(
        &pf_b.endpoint(),
        ca_pem.clone(),
        &token_url,
        &client_id,
        &client_secret,
        &audience,
        &scope,
    )
    .await;

    let not_found = |err: &udex_sdk::Error| matches!(err, udex_sdk::Error::Rpc(s) if s.code() == udex_sdk::grpc_code::NOT_FOUND);

    // 1. Index created on A is visible on B (describe + list).
    client_a
        .create_index(CreateIndexRequest {
            name: index_name.clone(),
            display_name: index_name.clone(),
            description: "k8s multi-instance test index".to_string(),
            max_bulk_operations: 100,
            max_key_length: 256,
            max_value_length: 1024,
            max_kv_pairs_per_context: 50,
            hash_algorithm: HashAlgorithm::Xxh3 as i32,
        })
        .await
        .expect("create_index on instance A failed");
    let on_b = client_b
        .describe_index(&index_name)
        .await
        .expect("describe_index on B failed — index created on A not visible to B");
    assert_eq!(on_b.name, index_name);
    let listed = client_b
        .list_indices()
        .await
        .expect("list_indices on B failed");
    assert!(
        listed.iter().any(|i| i.name == index_name),
        "index created on A missing from B's list_indices"
    );

    // 2. Entry written on A is readable on B.
    let created_ab = client_a
        .create_entry(&index_name, context_input(&[("cross_key_ab", "value_ab")]))
        .await
        .expect("create_entry on A failed");
    let key_on_b = client_b
        .lookup_key_by_context(&index_name, &created_ab.context_hash)
        .await
        .expect("lookup_key_by_context on B failed");
    assert_eq!(
        key_on_b.as_deref(),
        Some(created_ab.key.as_str()),
        "B did not resolve A's entry to the same key"
    );

    // 3. Entry written on B is readable on A.
    let created_ba = client_b
        .create_entry(&index_name, context_input(&[("cross_key_ba", "value_ba")]))
        .await
        .expect("create_entry on B failed");
    let key_on_a = client_a
        .lookup_key_by_context(&index_name, &created_ba.context_hash)
        .await
        .expect("lookup_key_by_context on A failed");
    assert_eq!(
        key_on_a.as_deref(),
        Some(created_ba.key.as_str()),
        "A did not resolve B's entry to the same key"
    );

    // 4. Deletes propagate both ways: delete A's entry on A → gone on B, and
    //    delete B's entry on B → gone on A. This also empties the index for (5).
    client_a
        .delete_entry(&index_name, &created_ab.key)
        .await
        .expect("delete_entry on A failed");
    let ab_on_b = client_b
        .lookup_context_by_key(&index_name, &created_ab.key)
        .await
        .expect_err("B still found an entry deleted on A");
    assert!(
        not_found(&ab_on_b),
        "expected NOT_FOUND on B after delete on A, got: {ab_on_b}"
    );

    client_b
        .delete_entry(&index_name, &created_ba.key)
        .await
        .expect("delete_entry on B failed");
    let ba_on_a = client_a
        .lookup_context_by_key(&index_name, &created_ba.key)
        .await
        .expect_err("A still found an entry deleted on B");
    assert!(
        not_found(&ba_on_a),
        "expected NOT_FOUND on A after delete on B, got: {ba_on_a}"
    );

    // 5. Index deleted on A is gone on B (index is now empty).
    client_a
        .delete_index(&index_name)
        .await
        .expect("delete_index on A failed");
    let idx_err = client_b
        .describe_index(&index_name)
        .await
        .expect_err("B still found an index deleted on A");
    assert!(
        not_found(&idx_err),
        "expected NOT_FOUND on B after delete_index on A, got: {idx_err}"
    );

    // pf_a / pf_b drop here → kubectl port-forward children are killed (RAII).
    drop((pf_a, pf_b));
}

// ── Observability tests (WP06) ──────────────────────────────────────────────
//
// These assert that telemetry from the k8s deployment lands in the local
// observability stack (Tempo / Prometheus / Loki). They reuse the k8s fixture
// (`data_k8s`), so they skip when `K8S_SERVER_URL` is unset, and additionally
// skip when the observability stack is not reachable. They are idempotent:
// trace assertions key off a freshly-created entry's unique server-generated key
// (a span attribute), and metric/log assertions are presence checks that hold
// after any traffic. Bring the stack up first:
//   bash projects/observability/scripts/up.sh
//
// The deployment exports telemetry to the host-published stack; from inside the
// devcontainer the backends are reached by service name. Override with the
// TEMPO_URL / PROMETHEUS_URL / LOKI_URL env vars for other environments.

fn obs_url(var: &str, default: &str) -> String {
    std::env::var(var).unwrap_or_else(|_| default.to_string())
}

fn now_unix_nanos() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_nanos()
}

// Returns true when Tempo, Prometheus, and Loki are all reachable, so the obs
// tests can run; false => skip (stack not up).
async fn obs_stack_ready() -> bool {
    let http = reqwest::Client::new();
    let checks = [
        (obs_url("TEMPO_URL", "http://tempo:3200"), "/ready"),
        (
            obs_url("PROMETHEUS_URL", "http://prometheus:9090"),
            "/-/ready",
        ),
        (obs_url("LOKI_URL", "http://loki:3100"), "/ready"),
    ];
    for (base, path) in checks {
        let ok = http
            .get(format!("{base}{path}"))
            .timeout(Duration::from_secs(3))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false);
        if !ok {
            return false;
        }
    }
    true
}

// Polls Tempo for a trace matching `traceql`, returning the sorted, de-duplicated
// span names of the first matching trace (empty if none within the budget).
async fn tempo_trace_span_names(traceql: &str) -> Vec<String> {
    let base = obs_url("TEMPO_URL", "http://tempo:3200");
    let http = reqwest::Client::new();
    for _ in 0..30u32 {
        if let Ok(resp) = http
            .get(format!("{base}/api/search"))
            .query(&[("q", traceql)])
            .timeout(Duration::from_secs(5))
            .send()
            .await
        {
            if let Ok(json) = resp.json::<serde_json::Value>().await {
                if let Some(trace_id) = json["traces"]
                    .as_array()
                    .and_then(|t| t.first())
                    .and_then(|t| t["traceID"].as_str())
                {
                    if let Ok(tr) = http
                        .get(format!("{base}/api/traces/{trace_id}"))
                        .timeout(Duration::from_secs(5))
                        .send()
                        .await
                    {
                        if let Ok(tj) = tr.json::<serde_json::Value>().await {
                            let mut names: Vec<String> = tj["batches"]
                                .as_array()
                                .into_iter()
                                .flatten()
                                .flat_map(|b| b["scopeSpans"].as_array().into_iter().flatten())
                                .flat_map(|s| s["spans"].as_array().into_iter().flatten())
                                .filter_map(|s| s["name"].as_str().map(str::to_string))
                                .collect();
                            names.sort();
                            names.dedup();
                            if !names.is_empty() {
                                return names;
                            }
                        }
                    }
                }
            }
        }
        sleep(Duration::from_secs(2)).await;
    }
    Vec::new()
}

// Polls a Prometheus instant query, returning the number of result series.
async fn prometheus_series_count(query: &str) -> usize {
    let base = obs_url("PROMETHEUS_URL", "http://prometheus:9090");
    let http = reqwest::Client::new();
    for _ in 0..30u32 {
        if let Ok(resp) = http
            .get(format!("{base}/api/v1/query"))
            .query(&[("query", query)])
            .timeout(Duration::from_secs(5))
            .send()
            .await
        {
            if let Ok(json) = resp.json::<serde_json::Value>().await {
                let n = json["data"]["result"].as_array().map_or(0, Vec::len);
                if n > 0 {
                    return n;
                }
            }
        }
        sleep(Duration::from_secs(2)).await;
    }
    0
}

// Reads a Prometheus instant query's first scalar value (e.g. the result of a
// `sum(...)`), returning None when there is no result.
async fn prometheus_scalar(query: &str) -> Option<f64> {
    let base = obs_url("PROMETHEUS_URL", "http://prometheus:9090");
    let http = reqwest::Client::new();
    let resp = http
        .get(format!("{base}/api/v1/query"))
        .query(&[("query", query)])
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .ok()?;
    let json = resp.json::<serde_json::Value>().await.ok()?;
    json["data"]["result"].as_array()?.first()?["value"]
        .as_array()?
        .get(1)?
        .as_str()?
        .parse::<f64>()
        .ok()
}

// Polls a Loki range query over a recent window, returning the total line count.
// A recent window (rather than the default wide range) keeps the check from
// matching telemetry left over from a much earlier run.
async fn loki_line_count(logql: &str) -> usize {
    let base = obs_url("LOKI_URL", "http://loki:3100");
    let http = reqwest::Client::new();
    for _ in 0..30u32 {
        let end = now_unix_nanos();
        let start = end.saturating_sub(600_000_000_000); // 10 min in ns
        if let Ok(resp) = http
            .get(format!("{base}/loki/api/v1/query_range"))
            .query(&[
                ("query", logql),
                ("start", &start.to_string()),
                ("end", &end.to_string()),
                ("limit", "100"),
            ])
            .timeout(Duration::from_secs(5))
            .send()
            .await
        {
            if let Ok(json) = resp.json::<serde_json::Value>().await {
                let n: usize = json["data"]["result"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|s| s["values"].as_array().map_or(0, Vec::len))
                    .sum();
                if n > 0 {
                    return n;
                }
            }
        }
        sleep(Duration::from_secs(2)).await;
    }
    0
}

#[rstest]
#[tokio_shared_rt::test]
async fn test_obs_k8s_traces_land() {
    let fixture = data_k8s().await;
    let Some((client, index_name)) = fixture.as_ref() else {
        return;
    };
    if !obs_stack_ready().await {
        eprintln!("observability stack not reachable — skipping test_obs_k8s_traces_land");
        return;
    }

    // A unique context yields a new entry with a server-generated key; the server
    // stamps that key onto the db.create_entry span as the `key` attribute, giving
    // us a run-specific handle to find this exact request's trace.
    let tag = format!("obs-{}", now_unix_nanos());
    let resp = client
        .create_entry(index_name, context_input(&[("obs_tag", &tag)]))
        .await
        .expect("create_entry for obs trace test");
    let key = resp.key;

    let spans = tempo_trace_span_names(&format!("{{ span.key = \"{key}\" }}")).await;
    assert!(
        !spans.is_empty(),
        "no trace found in Tempo for entry key {key}"
    );
    // The request span is named after the gRPC method; the datastore span is db.create_entry.
    assert!(
        spans.iter().any(|s| s.contains("CreateEntry")),
        "trace missing the CreateEntry request span; got {spans:?}"
    );
    assert!(
        spans.iter().any(|s| s == "db.create_entry"),
        "trace missing the db.create_entry span; got {spans:?}"
    );
}

#[rstest]
#[tokio_shared_rt::test]
async fn test_obs_k8s_metrics_land() {
    let fixture = data_k8s().await;
    let Some((client, _index_name)) = fixture.as_ref() else {
        return;
    };
    if !obs_stack_ready().await {
        eprintln!("observability stack not reachable — skipping test_obs_k8s_metrics_land");
        return;
    }

    // Run-specific check: capture the ListIndices counter, make the call, then poll
    // for an increase - so a stale series left from a prior run cannot satisfy the
    // test. The budget is generous because the OTel metric export interval (default
    // 60s) plus the Prometheus scrape interval gate how soon the increment appears.
    let method = "/udex.index.v1.IndexService/ListIndices";
    let query = format!(
        "sum(udex_rpc_requests_total{{deployment_environment=\"k3d\", rpc_method=\"{method}\"}})"
    );
    let baseline = prometheus_scalar(&query).await.unwrap_or(0.0);

    client
        .list_indices()
        .await
        .expect("list_indices for obs metric test");

    let mut increased = false;
    for _ in 0..60u32 {
        if let Some(v) = prometheus_scalar(&query).await {
            if v > baseline {
                increased = true;
                break;
            }
        }
        sleep(Duration::from_secs(2)).await;
    }
    assert!(
        increased,
        "udex_rpc_requests_total for {method} did not increase after the request (baseline {baseline})"
    );

    // PostgreSQL receiver metric — the Collector scrapes the DB continuously, so a
    // presence check is appropriate here.
    let pg = prometheus_series_count("postgresql_backends").await;
    assert!(pg > 0, "no postgresql_backends series in Prometheus");
}

#[rstest]
#[tokio_shared_rt::test]
async fn test_obs_k8s_logs_land() {
    let fixture = data_k8s().await;
    let Some((client, _index_name)) = fixture.as_ref() else {
        return;
    };
    if !obs_stack_ready().await {
        eprintln!("observability stack not reachable — skipping test_obs_k8s_logs_land");
        return;
    }

    // Generate activity so there are recent server logs to ship.
    client
        .list_indices()
        .await
        .expect("list_indices for obs log test");

    // The server's OTLP logs reach Loki tagged with the OTel service name.
    let lines = loki_line_count("{service_name=\"udex-server\"}").await;
    assert!(lines > 0, "no udex-server logs found in Loki");
}
