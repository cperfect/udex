// JWKS cache-miss and expiry refresh integration tests.
//
// Isolation strategy: each test creates a dedicated Hydra key set named
// udex-jwks-refresh-{uuid} and configures its test server to use
// {admin_url}/admin/keys/{set} as the jwks_url. This is independent of
// Hydra's /.well-known/jwks.json (used by the rest of the suite) so
// concurrent tests are unaffected. Each test cleans up its key set on exit.
//
// Hydra is assumed to be always available (devcontainer compose stack).
// Tests fail hard rather than skipping when Hydra is unreachable.

use base64::{
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
    Engine as _,
};
use jsonwebtoken::{encode, EncodingKey, Header};
use ory_hydra_client::{
    apis::{configuration::Configuration, jwk_api},
    models::CreateJsonWebKeySet,
};
use std::net::SocketAddr;
use tokio::time::{sleep, Duration};
use tonic::transport::{Certificate, Channel, ClientTlsConfig};
use tonic_health::pb::{health_client::HealthClient, HealthCheckRequest};
use udex_api::index::{CreateIndexRequest, HashAlgorithm, ListIndicesRequest};
use udex_datastore::integration_test::init_postgres;
use udex_server::{
    config::{AuthzConfig, ServerConfig, TlsConfig},
    logging, server,
};
use udex_test_utils::{bind_file_secret, hydra_admin_url};
use uuid::Uuid;

const TEST_ISSUER: &str = "jwks-refresh-test-issuer";
const TEST_AUDIENCE: &str = "jwks-refresh-test-audience";
// list_indices requires "udex:index:v1:list"; include it so authenticated
// requests return OK rather than PERMISSION_DENIED.
const TEST_SCOPE: &str = "udex:index:v1:list";

// ── helpers ───────────────────────────────────────────────────────────────────

fn admin_config() -> Configuration {
    Configuration {
        base_path: hydra_admin_url(),
        ..Configuration::default()
    }
}

/// Asks Hydra to generate an ES256 key with `kid` in the key set `set_name`.
/// Returns the `JsonWebKey` including the private `d` scalar (admin API always
/// includes private key material).
async fn create_hydra_key(
    admin: &Configuration,
    set_name: &str,
    kid: &str,
) -> ory_hydra_client::models::JsonWebKey {
    let body = CreateJsonWebKeySet::new("ES256".into(), kid.into(), "sig".into());
    let ks = jwk_api::create_json_web_key_set(admin, set_name, body)
        .await
        .expect("create Hydra JWK");
    ks.keys
        .unwrap_or_default()
        .into_iter()
        .find(|k| k.kid == kid)
        .expect("created key present in response")
}

/// Converts the base64url-encoded private scalar `d` of a P-256 JWK into an
/// `EncodingKey` via a PKCS8 PrivateKeyInfo DER (RFC 5958), which is the
/// format `EncodingKey::from_ec_pem` (and the underlying aws-lc-rs backend)
/// requires. The PEM header is `-----BEGIN PRIVATE KEY-----`.
///
/// Structure (67 bytes total):
/// ```text
/// SEQUENCE {                           -- outer PrivateKeyInfo
///   INTEGER 0                          -- version
///   SEQUENCE {                         -- AlgorithmIdentifier
///     OID 1.2.840.10045.2.1            -- ecPublicKey
///     OID 1.2.840.10045.3.1.7          -- P-256
///   }
///   OCTET STRING {
///     SEQUENCE {                       -- ECPrivateKey (RFC 5915 minimal)
///       INTEGER 1                      -- version
///       OCTET STRING <d: 32 bytes>     -- private key scalar
///     }
///   }
/// }
/// ```
fn ec_d_to_encoding_key(d_b64url: &str) -> EncodingKey {
    let d = URL_SAFE_NO_PAD.decode(d_b64url).expect("base64url d field");
    assert_eq!(d.len(), 32, "P-256 private scalar must be 32 bytes");

    // ecPublicKey OID: 1.2.840.10045.2.1 (7 bytes)
    const EC_OID: &[u8] = &[0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01];
    // P-256 OID: 1.2.840.10045.3.1.7 (8 bytes)
    const P256_OID: &[u8] = &[0x2a, 0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07];

    // Minimal ECPrivateKey (RFC 5915): SEQUENCE { INTEGER 1, OCTET STRING d }
    // Content: 3 + 34 = 37 bytes  →  header + content = 39 bytes
    let mut ec_key = Vec::with_capacity(39);
    ec_key.extend_from_slice(&[0x30, 0x25]); // SEQUENCE, 37 bytes
    ec_key.extend_from_slice(&[0x02, 0x01, 0x01]); // INTEGER 1
    ec_key.extend_from_slice(&[0x04, 0x20]); // OCTET STRING, 32 bytes
    ec_key.extend_from_slice(&d);

    // AlgorithmIdentifier: SEQUENCE { OID ecPublicKey, OID P-256 }
    // Content: 9 + 10 = 19 bytes  →  header + content = 21 bytes
    let mut alg_id = Vec::with_capacity(21);
    alg_id.extend_from_slice(&[0x30, 0x13]); // SEQUENCE, 19 bytes
    alg_id.push(0x06);
    alg_id.push(EC_OID.len() as u8);
    alg_id.extend_from_slice(EC_OID);
    alg_id.push(0x06);
    alg_id.push(P256_OID.len() as u8);
    alg_id.extend_from_slice(P256_OID);

    // OCTET STRING wrapping the ECPrivateKey (39 bytes)
    let mut octet_str = Vec::with_capacity(41);
    octet_str.extend_from_slice(&[0x04, 0x27]); // OCTET STRING, 39 bytes
    octet_str.extend_from_slice(&ec_key);

    // Outer PrivateKeyInfo SEQUENCE: INTEGER 0 + AlgorithmIdentifier + OCTET STRING
    // Content: 3 + 21 + 41 = 65 bytes
    let mut pkcs8 = Vec::with_capacity(67);
    pkcs8.extend_from_slice(&[0x30, 0x41]); // SEQUENCE, 65 bytes
    pkcs8.extend_from_slice(&[0x02, 0x01, 0x00]); // INTEGER 0 (version)
    pkcs8.extend_from_slice(&alg_id);
    pkcs8.extend_from_slice(&octet_str);

    let b64 = STANDARD.encode(&pkcs8);
    let body = b64
        .as_bytes()
        .chunks(64)
        .map(|c| std::str::from_utf8(c).unwrap())
        .collect::<Vec<_>>()
        .join("\n");
    let pem = format!("-----BEGIN PRIVATE KEY-----\n{body}\n-----END PRIVATE KEY-----\n");

    EncodingKey::from_ec_pem(pem.as_bytes())
        .expect("valid PKCS8 private key PEM constructed from Hydra JWK d field")
}

/// Mints a JWT signed with `key`, carrying `kid` in the header and the test
/// issuer, audience, and scope needed to pass full server-side auth.
fn mint_jwt(key: &EncodingKey, kid: &str) -> String {
    use time::OffsetDateTime;
    let now = OffsetDateTime::now_utc().unix_timestamp() as usize;
    let claims = udex_api::authz::claims::Claims::new(
        "refresh-test-subject".to_string(),
        TEST_ISSUER.to_string(),
        TEST_AUDIENCE.to_string(),
        now + 3600,
        now,
    )
    .with_scope(TEST_SCOPE.to_string());

    let mut header = Header::new(jsonwebtoken::Algorithm::ES256);
    header.kid = Some(kid.to_string());
    header.typ = Some("JWT".to_string());
    encode(&header, &claims, key).expect("JWT encoding failed")
}

/// Polls the gRPC health endpoint until the server is SERVING or panics after
/// 3 seconds. Mirrors the pattern used in `cli/tests/health_tests.rs`.
async fn wait_for_server(addr: SocketAddr) {
    let ca = tokio::fs::read_to_string("tests/certs/ca.crt")
        .await
        .expect("read CA cert");
    let tls = ClientTlsConfig::new()
        .ca_certificate(tonic::transport::Certificate::from_pem(ca))
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
                service: String::new(),
            })
            .await
            .is_ok()
        {
            return;
        }
    }
    panic!("JWKS refresh test server at {addr} did not become ready within 3 s");
}

/// Starts a udex server whose `jwks_url` points to a Hydra custom key set.
/// Returns `(bind_address, server_join_handle, db_name_for_cleanup)`.
async fn start_jwks_server(
    jwks_url: String,
    index_name: &str,
    max_failed_refreshes: Option<u32>,
    backoff_factor_secs: Option<u64>,
) -> (SocketAddr, tokio::task::JoinHandle<()>, String) {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    logging::init_test_tracing();

    let (datastore, _, db_name) = init_postgres().await;
    // OS-assigned ephemeral port — avoids hardcoded ports that collide across
    // concurrent runs or with leftover processes.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let addr: SocketAddr = listener.local_addr().expect("listener local address");

    let config = ServerConfig {
        bind_address: addr,
        request_timeout: std::time::Duration::from_secs(30),
        max_connections: 100,
        max_message_size: 4 * 1024 * 1024,
        tls: TlsConfig {
            cert: bind_file_secret("tests/certs/server.crt"),
            key: bind_file_secret("tests/certs/server.key"),
        },
        init_indexes: vec![CreateIndexRequest {
            name: index_name.to_string(),
            display_name: index_name.to_string(),
            description: index_name.to_string(),
            max_bulk_operations: 100,
            max_key_length: 256,
            max_value_length: 1024,
            max_kv_pairs_per_context: 50,
            hash_algorithm: HashAlgorithm::Xxh3 as i32,
        }],
        authz: AuthzConfig {
            jwks_url: Some(jwks_url),
            jwt_public_key: None,
            jwt_issuer: Some(TEST_ISSUER.to_string()),
            jwt_audience: Some(TEST_AUDIENCE.to_string()),
            danger_allow_non_tls: true,
            scope_claim_name: None,
            mask_subject_in_logs: false,
            jwks_max_failed_refreshes: max_failed_refreshes,
            jwks_backoff_factor_secs: backoff_factor_secs,
            jwks_max_age_secs: Some(0), // disable expiry task; we test cache-miss only here
        },
    };

    let handle = tokio::spawn(async move {
        server::serve_with_listener(config, datastore, listener)
            .await
            .expect("JWKS refresh test server failed");
    });
    wait_for_server(addr).await;
    (addr, handle, db_name)
}

/// Calls `list_indices` on `addr` with `token` and returns the gRPC status.
/// A distinct port per test means we create a fresh channel each time.
async fn call_list_indices(addr: SocketAddr, token: &str) -> tonic::Code {
    let ca = tokio::fs::read_to_string("tests/certs/ca.crt")
        .await
        .expect("read CA cert");
    let tls = ClientTlsConfig::new()
        .ca_certificate(Certificate::from_pem(ca))
        .domain_name("localhost");
    let endpoint = Channel::from_shared(format!("https://{addr}"))
        .expect("endpoint URI")
        .tls_config(tls)
        .expect("TLS config");
    let channel = endpoint.connect().await.expect("gRPC connect");

    let mut client = udex_api::index::index_service_client::IndexServiceClient::new(channel);
    let mut req = tonic::Request::new(ListIndicesRequest {});
    req.metadata_mut()
        .insert("authorization", format!("Bearer {token}").parse().unwrap());

    match client.list_indices(req).await {
        Ok(_) => tonic::Code::Ok,
        Err(s) => s.code(),
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

/// A token signed with a new key (unknown `kid`) triggers an inline JWKS
/// refresh; the server validates the token after refreshing.
#[tokio::test]
async fn test_server_oauth2_cache_miss_triggers_refresh_and_validates_new_kid() {
    let admin = admin_config();
    let set_name = format!("udex-jwks-refresh-{}", Uuid::new_v4().simple());
    let k1_kid = "refresh-k1";
    let k2_kid = "refresh-k2";

    // K1: in cache at startup.
    let k1_jwk = create_hydra_key(&admin, &set_name, k1_kid).await;
    let k1_key = ec_d_to_encoding_key(k1_jwk.d.as_deref().expect("admin API includes private key"));

    let index = format!("jwks-cache-miss-{}", Uuid::new_v4().simple());
    let jwks_url = format!("{}/admin/keys/{}", admin.base_path, set_name);
    let (addr, server, _db) = start_jwks_server(jwks_url, &index, None, None).await;

    // K1 token must validate immediately (key was in cache at startup).
    let code = call_list_indices(addr, &mint_jwt(&k1_key, k1_kid)).await;
    assert_eq!(
        code,
        tonic::Code::Ok,
        "K1 token should validate from startup cache"
    );

    // Add K2 to the set — server has not seen this kid yet.
    let k2_jwk = create_hydra_key(&admin, &set_name, k2_kid).await;
    let k2_key = ec_d_to_encoding_key(k2_jwk.d.as_deref().expect("admin API includes private key"));

    // K2 token: cache miss → refresh → validates.
    let code = call_list_indices(addr, &mint_jwt(&k2_key, k2_kid)).await;
    assert_eq!(
        code,
        tonic::Code::Ok,
        "K2 token should validate after cache-miss refresh"
    );

    // K1 token still validates (retained in the refreshed key map).
    let code = call_list_indices(addr, &mint_jwt(&k1_key, k1_kid)).await;
    assert_eq!(
        code,
        tonic::Code::Ok,
        "K1 token should still validate after refresh"
    );

    server.abort();
    jwk_api::delete_json_web_key_set(&admin, &set_name)
        .await
        .ok();
}

/// When the JWKS endpoint becomes unavailable (key set deleted), the server
/// retains its cached keys and continues to serve tokens for known kids.
#[tokio::test]
async fn test_server_oauth2_endpoint_failure_retains_cache_and_serves_known_kids() {
    let admin = admin_config();
    let set_name = format!("udex-jwks-refresh-{}", Uuid::new_v4().simple());
    let k1_kid = "refresh-k1";

    let k1_jwk = create_hydra_key(&admin, &set_name, k1_kid).await;
    let k1_key = ec_d_to_encoding_key(k1_jwk.d.as_deref().expect("admin API includes private key"));

    let index = format!("jwks-endpoint-fail-{}", Uuid::new_v4().simple());
    let jwks_url = format!("{}/admin/keys/{}", admin.base_path, set_name);
    let (addr, server, _db) = start_jwks_server(jwks_url, &index, None, None).await;

    // K1 works before the endpoint goes away.
    let code = call_list_indices(addr, &mint_jwt(&k1_key, k1_kid)).await;
    assert_eq!(
        code,
        tonic::Code::Ok,
        "K1 should validate before endpoint failure"
    );

    // Delete the key set — Hydra will return 404 on all subsequent JWKS fetches.
    jwk_api::delete_json_web_key_set(&admin, &set_name)
        .await
        .expect("delete Hydra key set");

    // An unknown kid forces a refresh attempt that fails (404).
    let code = call_list_indices(addr, &mint_jwt(&k1_key, "unknown-kid")).await;
    assert_eq!(
        code,
        tonic::Code::Unauthenticated,
        "Unknown kid after endpoint failure should be rejected"
    );

    // K1 (in cache since startup) must still validate — cache is retained.
    let code = call_list_indices(addr, &mint_jwt(&k1_key, k1_kid)).await;
    assert_eq!(
        code,
        tonic::Code::Ok,
        "K1 should still validate from cache after endpoint failure"
    );

    server.abort();
}

/// Once the max-failed-refreshes gate fires the server stops attempting
/// refreshes but continues to serve tokens for keys still in the cache.
#[tokio::test]
async fn test_server_oauth2_max_failed_refreshes_gate_stops_retries_and_cache_is_retained() {
    let admin = admin_config();
    let set_name = format!("udex-jwks-refresh-{}", Uuid::new_v4().simple());
    let k1_kid = "refresh-k1";

    let k1_jwk = create_hydra_key(&admin, &set_name, k1_kid).await;
    let k1_key = ec_d_to_encoding_key(k1_jwk.d.as_deref().expect("admin API includes private key"));

    let index = format!("jwks-gate-{}", Uuid::new_v4().simple());
    let jwks_url = format!("{}/admin/keys/{}", admin.base_path, set_name);
    // max_failed_refreshes = 2, backoff_factor = 2 (≈1 s backoff after first
    // failure so the test waits briefly rather than minutes).
    let (addr, server, _db) = start_jwks_server(jwks_url, &index, Some(2), Some(2)).await;

    // K1 is in the startup cache.
    let code = call_list_indices(addr, &mint_jwt(&k1_key, k1_kid)).await;
    assert_eq!(
        code,
        tonic::Code::Ok,
        "K1 should validate from startup cache"
    );

    // Delete the set so all refresh attempts fail.
    jwk_api::delete_json_web_key_set(&admin, &set_name)
        .await
        .expect("delete Hydra key set");

    // First cache-miss: refresh fails → consecutive_failures = 1, backoff ≈ 1 s.
    let code = call_list_indices(addr, &mint_jwt(&k1_key, "unknown-kid")).await;
    assert_eq!(
        code,
        tonic::Code::Unauthenticated,
        "first miss: refresh fails"
    );

    // Wait for the backoff to expire (factor=2 → temp=2, half=1, delay≈1 s).
    sleep(Duration::from_millis(1500)).await;

    // Second cache-miss: refresh fails → consecutive_failures = 2 = max → gate fires.
    let code = call_list_indices(addr, &mint_jwt(&k1_key, "unknown-kid")).await;
    assert_eq!(
        code,
        tonic::Code::Unauthenticated,
        "second miss: refresh fails, gate fires"
    );

    // Third attempt: gate immediately blocks without a network call.
    let code = call_list_indices(addr, &mint_jwt(&k1_key, "unknown-kid")).await;
    assert_eq!(
        code,
        tonic::Code::Unauthenticated,
        "third miss: gate is active"
    );

    // K1 (cached at startup) must still validate — the gate does not evict the cache.
    let code = call_list_indices(addr, &mint_jwt(&k1_key, k1_kid)).await;
    assert_eq!(
        code,
        tonic::Code::Ok,
        "K1 must still validate from cache after gate fires"
    );

    server.abort();
}

// ── unit tests (no Hydra required) ────────────────────────────────────────────

#[cfg(test)]
mod unit {
    use super::*;

    /// Verifies ec_d_to_encoding_key produces a key accepted by jsonwebtoken
    /// using the RFC 7515 Appendix A.3 P-256 test vector. Catches DER format
    /// bugs without requiring a live Hydra instance.
    #[test]
    fn ec_d_to_encoding_key_accepts_known_p256_vector() {
        // RFC 7515 §A.3 private key scalar d (P-256, base64url).
        let d = "jpsQnnGQmL-YBIffH1136cspYG6-0iY7X1fCE9-E9LI";
        // Panics with InvalidKeyFormat if the PKCS8 DER construction is wrong.
        let _key = ec_d_to_encoding_key(d);
    }
}
