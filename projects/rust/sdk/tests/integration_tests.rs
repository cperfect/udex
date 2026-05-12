//! Integration tests for `udex-sdk`.
//!
//! Two fixtures are provided:
//!
//! 1. **JWT fixture** (`data()`) — always runs. Spins up an embedded server
//!    with static-JWT auth and connects an SDK client via
//!    [`ClientOptions::static_bearer_token`].
//!
//! 2. **Hydra fixture** (`data_hydra()`) — requires a running Hydra instance
//!    (set by `HYDRA_ADMIN_URL` / `HYDRA_PUBLIC_URL` in `.env`). Connects the
//!    SDK using [`ClientOptions::client_credentials`] so the full
//!    `TokenManager` lifecycle is exercised.

use std::net::SocketAddr;
use std::sync::OnceLock;

use jsonwebtoken::{encode, EncodingKey, Header};
use maybe_once::tokio::{Data, MaybeOnceAsync};
use rstest::*;
use secrets_rs::{sources::file::FileSource, Secret, SourceRegistry};
use time::OffsetDateTime;
use tokio::time::{sleep, Duration};
use tonic::transport::{Certificate, Channel, ClientTlsConfig};
use udex_api::healthz::{healthz_service_client::HealthzServiceClient, HealthzRequest};
use udex_api::index::{HashAlgorithm, IndexUpdate, UpdateIndexRequest};
use udex_datastore::integration_test::init_postgres;
use udex_sdk::{ClientOptions, ContextInput, KeyValuePair, UdexClient, Value};

// ── Port constants ────────────────────────────────────────────────────────────
// Different from the server crate's own tests to avoid port conflicts when both
// run concurrently.

const SDK_JWT_BIND_ADDR: &str = "127.0.0.1:50054";
const SDK_HYDRA_BIND_ADDR: &str = "127.0.0.1:15055";
const ID_PREFIX: &str = "sdk-integration-test";
const ID_HYDRA_PREFIX: &str = "sdk-hydra-integration-test";

// ── Cert / JWT key paths ──────────────────────────────────────────────────────
// CARGO_MANIFEST_DIR points to the sdk/ package root at compile time.
// The server's test fixtures live one level up under server/tests/.

const CERTS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../server/tests/certs");
const JWT_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../server/tests/jwt");

fn server_cert_path(file: &str) -> String {
    format!("{CERTS_DIR}/{file}")
}

fn jwt_key_path(file: &str) -> String {
    format!("{JWT_DIR}/{file}")
}

/// Binds a file-sourced `Secret<String>` from an absolute path.
fn bind_file_secret(abs_path: &str) -> Secret<String> {
    let mut s = Secret::new(&format!("urn:secrets-rs:file:{abs_path}")).expect("valid file URN");
    let mut reg = SourceRegistry::new();
    reg.register("file", FileSource::new())
        .expect("register file source");
    s.bind(&reg).expect("bind file secret");
    s
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
        if HealthzServiceClient::new(ch)
            .healthz(tonic::Request::new(HealthzRequest {}))
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
        init_indexes: vec![UpdateIndexRequest {
            name: index_name.clone(),
            update: Some(IndexUpdate {
                description: Some(index_name.clone()),
                max_bulk_operations: Some(100),
                max_key_length: Some(256),
                max_value_length: Some(1024),
                max_kv_pairs_per_context: Some(50),
                hash_algorithm: Some(HashAlgorithm::Xxh3 as i32),
            }),
        }],
        authz: udex_server::config::AuthzConfig {
            jwks_url: None,
            jwt_public_key: Some(bind_file_secret(&jwt_key_path("signing_public_key.pem"))),
            jwt_issuer: Some(jwt_issuer.clone()),
            jwt_audience: Some(jwt_audience.clone()),
            danger_allow_non_tls: false,
            scope_claim_name: None,
            mask_subject_in_logs: false,
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

    // Sign a JWT that grants full access to the test index.
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

fn hydra_public_url() -> String {
    dotenvy::dotenv_override().ok();
    std::env::var("HYDRA_PUBLIC_URL").unwrap_or_else(|_| "http://localhost:4444".to_string())
}

fn hydra_admin_url() -> String {
    dotenvy::dotenv_override().ok();
    std::env::var("HYDRA_ADMIN_URL").unwrap_or_else(|_| "http://localhost:4445".to_string())
}

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
        init_indexes: vec![UpdateIndexRequest {
            name: index_name.clone(),
            update: Some(IndexUpdate {
                description: Some(index_name.clone()),
                max_bulk_operations: Some(100),
                max_key_length: Some(256),
                max_value_length: Some(1024),
                max_kv_pairs_per_context: Some(50),
                hash_algorithm: Some(HashAlgorithm::Xxh3 as i32),
            }),
        }],
        authz: udex_server::config::AuthzConfig {
            jwks_url: Some(jwks_url),
            jwt_public_key: None,
            jwt_issuer: Some(issuer),
            jwt_audience: Some(audience.clone()),
            danger_allow_non_tls: true,
            scope_claim_name: Some("scp".to_string()),
            mask_subject_in_logs: false,
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

    // Register a Hydra client that has all the scopes needed by the test index.
    register_hydra_client(
        &admin_url,
        &client_id,
        &client_secret,
        &audience,
        &index_name,
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
                 udex:index:v1:{index_name}:read \
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
             udex:index:v1:{index_name}:read \
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
            })
            .collect(),
        dek: None,
        kek_id: None,
    }
}

/// Registers (or replaces) a Hydra client with all scopes for `index_name`.
async fn register_hydra_client(
    admin_url: &str,
    client_id: &str,
    client_secret: &str,
    audience: &str,
    index_name: &str,
) {
    use ory_hydra_client::apis::{configuration::Configuration, o_auth2_api};
    use ory_hydra_client::models::o_auth2_client::OAuth2Client as HydraClient;

    let config = Configuration {
        base_path: admin_url.to_string(),
        ..Configuration::default()
    };

    let scopes = format!(
        "udex:index:v1:list \
         udex:index:v1:{index_name}:read \
         udex:entry:v1:{index_name}:create \
         udex:entry:v1:{index_name}:read \
         udex:entry:v1:{index_name}:write \
         udex:entry:v1:{index_name}:delete"
    );

    let mut body = HydraClient::new();
    body.access_token_strategy = Some("jwt".to_string());
    body.audience = Some(vec![audience.to_string()]);
    body.client_id = Some(client_id.to_string());
    body.client_name = Some(client_id.to_string());
    body.client_secret = Some(client_secret.to_string());
    body.grant_types = Some(vec!["client_credentials".to_string()]);
    body.scope = Some(scopes);
    body.token_endpoint_auth_method = Some("client_secret_post".to_string());

    match o_auth2_api::create_o_auth2_client(&config, body.clone()).await {
        Ok(_) => {}
        Err(e)
            if matches!(
                &e,
                ory_hydra_client::apis::Error::ResponseError(r) if r.status.as_u16() == 409
            ) =>
        {
            o_auth2_api::set_o_auth2_client(&config, client_id, body)
                .await
                .expect("Hydra set_client failed");
        }
        Err(e) => panic!("Hydra create_client failed: {e}"),
    }
}

// ── JWT-backed tests (always run) ─────────────────────────────────────────────

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

#[rstest]
#[tokio_shared_rt::test]
async fn test_sdk_invalid_token_returns_rpc_error() {
    // Ensure the JWT fixture server is started before connecting.
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

    // Build a context with one plaintext pair and one envelope-encrypted pair.
    let ctx = ContextInput {
        pairs: vec![
            KeyValuePair {
                key: "user_id".to_string(),
                value: Some(Value {
                    value: Some(udex_sdk::value::Value::StringValue("42".to_string())),
                }),
                kek_id: None,
            },
            KeyValuePair {
                key: "email".to_string(),
                value: Some(Value {
                    value: Some(udex_sdk::value::Value::StringValue(encrypted_value)),
                }),
                kek_id: Some(kek_id.to_string()),
            },
        ],
        dek: Some(encrypted_dek),
        kek_id: Some(kek_id.to_string()),
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

    // The server must echo back kek_id and the wrapped DEK unchanged.
    assert_eq!(found_ctx.kek_id.as_deref(), Some(kek_id));
    let returned_encrypted_dek = found_ctx.dek.as_deref().expect("missing dek");

    // Unwrap the DEK using the KEK.
    let dek_bytes_enc = B64
        .decode(returned_encrypted_dek)
        .expect("base64 decode DEK");
    let (dek_nonce_bytes, dek_ct_bytes) = dek_bytes_enc.split_at(12);
    let unwrapped_dek = kek
        .decrypt(Nonce::from_slice(dek_nonce_bytes), dek_ct_bytes)
        .expect("decrypt DEK");
    let dek_dec = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&unwrapped_dek));

    // Find the encrypted pair and verify the kek_id was echoed back.
    let email_pair = found_ctx
        .pairs
        .iter()
        .find(|p| p.key == "email")
        .expect("email pair missing");
    assert_eq!(email_pair.kek_id.as_deref(), Some(kek_id));

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
async fn test_hydra_sdk_list_and_describe_index() {
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
async fn test_hydra_sdk_create_and_lookup_entry() {
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
async fn test_hydra_sdk_create_entry_idempotent() {
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
async fn test_hydra_sdk_delete_entry() {
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
async fn test_hydra_sdk_lookup_nonexistent_returns_none() {
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
async fn test_hydra_sdk_bulk_write_and_read() {
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
async fn test_hydra_sdk_invalid_credentials_return_auth_error() {
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
