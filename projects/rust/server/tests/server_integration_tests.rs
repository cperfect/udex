#[path = "auth_server.rs"]
mod auth_server;

use jsonwebtoken::{encode, EncodingKey, Header};
// we don't test all the services exhaustively here as they will be tested via the client & end-to-end tests
use maybe_once::tokio::{Data, MaybeOnceAsync};
use std::net::SocketAddr;
use std::sync::OnceLock;
use time::OffsetDateTime;
use tokio::net::TcpListener;
use tokio::time::{sleep, Duration};
use tonic::transport::{Channel, ClientTlsConfig};
use tonic_health::pb::{
    health_check_response::ServingStatus, health_client::HealthClient, HealthCheckRequest,
};
use udex_api::index::{CreateIndexRequest, HashAlgorithm};
use udex_datastore::integration_test::init_postgres;
use udex_server::{config::ServerConfig, logging, server};
use udex_test_utils::{bind_file_secret, hydra_admin_url};

const ID_PREFIX: &str = "server-integration-test";

const ID_USING_HYDRA_PREFIX: &str = "hydra-integration-test";

/// Binds an OS-assigned ephemeral port on localhost and returns the listener
/// plus its actual address. Using port 0 avoids hardcoded ports that collide
/// across concurrent runs or with leftover processes.
async fn bind_ephemeral() -> (TcpListener, SocketAddr) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("failed to bind ephemeral port");
    let addr = listener
        .local_addr()
        .expect("listener has no local address");
    (listener, addr)
}

/// Polls the gRPC health endpoint over TLS until the server reports SERVING,
/// or panics after ~3 s. Replaces fixed startup sleeps so the fixtures are not
/// sensitive to how long startup takes under load.
async fn wait_for_server(addr: SocketAddr) {
    let ca = tokio::fs::read_to_string("tests/certs/ca.crt")
        .await
        .expect("read CA cert");
    let tls = ClientTlsConfig::new()
        .ca_certificate(tonic::transport::Certificate::from_pem(ca))
        .domain_name("localhost");
    for _ in 0..30 {
        sleep(Duration::from_millis(100)).await;
        let Ok(channel) = Channel::from_shared(format!("https://{addr}"))
            .unwrap()
            .tls_config(tls.clone())
            .unwrap()
            .connect()
            .await
        else {
            continue;
        };
        if HealthClient::new(channel)
            .check(HealthCheckRequest {
                service: String::new(),
            })
            .await
            .is_ok()
        {
            return;
        }
    }
    panic!("server at {addr} did not become ready within 3 s");
}

// See https://github.com/ufoscout/maybe-once for the MaybeOnceAsync pattern used here.
type MaybeOnceType = (
    String,                      // 0: index name
    SocketAddr,                  // 1: bind address
    EncodingKey,                 // 2: jwt signing key for generating valid tokens
    EncodingKey,                 // 3: bad signing key for testing invalid signatures
    tokio::task::JoinHandle<()>, // 4: server task handle so the server doesn't get dropped
    String,                      // 5: database name for cleanup
    String,                      // 6: jwt_issuer
    String,                      // 7: jwt_audience
);

/// Initializer function.
/// Starts a Postgres container shared between all tests.
/// It will be stopped when the tests terminate.
async fn init_server() -> MaybeOnceType {
    logging::init_test_tracing();

    // Initialize rustls crypto provider
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    println!("Initializing server...");

    let datastore_fixtures = init_postgres().await;
    let datastore = datastore_fixtures.0;
    let db_name = datastore_fixtures.2;

    let index_name = format!("{}-index", ID_PREFIX);
    let (listener, bind_address) = bind_ephemeral().await;
    let jwt_issuer = format!("{ID_PREFIX}-issuer");
    let jwt_audience = format!("{ID_PREFIX}-audience");

    let server_config = ServerConfig {
        bind_address,
        request_timeout: std::time::Duration::from_secs(30),
        max_connections: 1000,
        max_message_size: 4 * 1024 * 1024,
        tls: udex_server::config::TlsConfig {
            cert: bind_file_secret("tests/certs/server.crt"),
            key: bind_file_secret("tests/certs/server.key"),
        },
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
            jwt_public_key: Some(bind_file_secret("tests/jwt/signing_public_key.pem")),
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

    let server_handle = tokio::spawn(async move {
        server::serve_with_listener(server_config, datastore, listener)
            .await
            .expect("Server failed to start");
    });

    // Wait until the server reports SERVING rather than sleeping a fixed time.
    wait_for_server(bind_address).await;

    let jwt_private_key = tokio::fs::read_to_string("tests/jwt/signing_private_key.pem")
        .await
        .expect("Failed to read JWT private signing key");
    let jwt_signing_key = EncodingKey::from_ec_pem(jwt_private_key.as_bytes())
        .expect("Failed to create EncodingKey from private key");

    let bad_private_key = tokio::fs::read_to_string("tests/jwt/bad_signing_private_key.pem")
        .await
        .expect("Failed to read bad private key");
    let bad_signing_key = EncodingKey::from_ec_pem(bad_private_key.as_bytes())
        .expect("Failed to create bad EncodingKey from bad private key");

    (
        index_name,
        bind_address,
        jwt_signing_key,
        bad_signing_key,
        server_handle,
        db_name,
        jwt_issuer,
        jwt_audience,
    )
}

/// A function that holds a static reference to the container
pub async fn data(serial: bool) -> Data<'static, MaybeOnceType> {
    static DATA: OnceLock<MaybeOnceAsync<MaybeOnceType>> = OnceLock::new();
    DATA.get_or_init(|| MaybeOnceAsync::new(|| Box::pin(init_server())))
        .data(serial)
        .await
}

// ── Hydra JWKS fixture ────────────────────────────────────────────────────────
//
// Activated when HYDRA_ADMIN_URL (and optionally HYDRA_PUBLIC_URL) env vars are
// set. Starts a second server instance that validates tokens via Hydra's JWKS
// endpoint. Tests that depend on this fixture skip gracefully when the env vars
// are absent.

type HydraFixtureType = (
    String,                         // 0: index name
    SocketAddr,                     // 1: bind address
    auth_server::OAuthClientConfig, // 2: Hydra client credentials for test token requests
    String,                         // 3: Hydra public base URL
    tokio::task::JoinHandle<()>,    // 4: server task handle (keeps server alive)
    String,                         // 5: database name (kept for automatic cleanup)
);

async fn init_server_hydra() -> HydraFixtureType {
    let admin_url = hydra_admin_url();
    let public_url =
        std::env::var("HYDRA_PUBLIC_URL").unwrap_or_else(|_| "http://localhost:4444".to_string());
    // URLS_SELF_ISSUER in the docker-compose config has a trailing slash.
    let issuer = std::env::var("HYDRA_ISSUER")
        .unwrap_or_else(|_| format!("{}/", public_url.trim_end_matches('/')));
    let audience = format!("{ID_USING_HYDRA_PREFIX}-audience");
    let jwks_url = format!("{public_url}/.well-known/jwks.json");

    logging::init_test_tracing();
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let datastore_fixtures = init_postgres().await;
    let datastore = datastore_fixtures.0;
    let db_name = datastore_fixtures.2;

    let index_name = format!("{ID_USING_HYDRA_PREFIX}-index");
    let (listener, bind_address) = bind_ephemeral().await;

    let server_config = ServerConfig {
        bind_address,
        request_timeout: std::time::Duration::from_secs(30),
        max_connections: 1000,
        max_message_size: 4 * 1024 * 1024,
        tls: udex_server::config::TlsConfig {
            cert: bind_file_secret("tests/certs/server.crt"),
            key: bind_file_secret("tests/certs/server.key"),
        },
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
            // Local Hydra runs over plain HTTP; allow it in test-only config.
            danger_allow_non_tls: true,
            // Hydra uses "scp" (array) instead of the RFC 8693 default "scope".
            scope_claim_name: Some("scp".to_string()),
            mask_subject_in_logs: false,
            jwks_max_failed_refreshes: None,
            jwks_backoff_factor_secs: None,
            jwks_max_age_secs: None,
        },
    };

    let server_handle = tokio::spawn(async move {
        server::serve_with_listener(server_config, datastore, listener)
            .await
            .expect("Hydra-backed server failed to start");
    });

    // Wait until the server reports SERVING. This also covers the extra startup
    // time the AuthzInterceptor needs to fetch the JWKS on startup.
    wait_for_server(bind_address).await;

    // One shared Hydra client for all Hydra integration tests.
    // Scopes cover all operations tested across the Hydra test suite.
    let client_config = auth_server::OAuthClientConfig {
        name: format!("{ID_USING_HYDRA_PREFIX}-client"),
        id: format!("{ID_USING_HYDRA_PREFIX}-client"),
        secret: "hydra-test-secret".to_string(),
        scopes: vec![
            format!("udex:index:v1:{index_name}:read"),
            format!("udex:entry:v1:{index_name}:create"),
        ],
        audience,
    };

    auth_server::create_oauth2_client(&admin_url, client_config.clone())
        .await
        .expect("Failed to create Hydra OAuth2 client");

    (
        index_name,
        bind_address,
        client_config,
        public_url,
        server_handle,
        db_name,
    )
}

pub async fn data_using_hydra(serial: bool) -> Data<'static, HydraFixtureType> {
    static HYDRA_DATA: OnceLock<MaybeOnceAsync<HydraFixtureType>> = OnceLock::new();
    HYDRA_DATA
        .get_or_init(|| MaybeOnceAsync::new(|| Box::pin(init_server_hydra())))
        .data(serial)
        .await
}

struct OverrideClaims {
    sub: Option<String>,
    issuer: Option<String>,
    audience: Option<String>,
    exp: Option<usize>,
    iat: Option<usize>,
    scope: Option<String>,
}

/// Generates claims for testing JWT authentication.
///
/// `issuer` and `audience` are the expected values from the server config.
/// `override_claims` allows individual fields to be overridden for error-path tests.
fn generate_test_claims(
    sub: &str,
    issuer: &str,
    audience: &str,
    override_claims: Option<OverrideClaims>,
) -> udex_api::authz::claims::Claims {
    let now = OffsetDateTime::now_utc().unix_timestamp() as usize;
    let mut claims = udex_api::authz::claims::Claims::new(
        override_claims
            .as_ref()
            .and_then(|c| c.sub.clone())
            .unwrap_or_else(|| sub.to_string()),
        override_claims
            .as_ref()
            .and_then(|c| c.issuer.clone())
            .unwrap_or_else(|| issuer.to_string()),
        override_claims
            .as_ref()
            .and_then(|c| c.audience.clone())
            .unwrap_or_else(|| audience.to_string()),
        override_claims
            .as_ref()
            .and_then(|c| c.exp)
            .unwrap_or_else(|| now + 3600),
        override_claims.as_ref().and_then(|c| c.iat).unwrap_or(now),
    );
    if let Some(override_claims) = override_claims {
        if let Some(scope) = override_claims.scope {
            claims = claims.with_scope(scope);
        }
    }
    claims
}

/// Generates a test jwt using the given, claims and signing key
fn generate_test_jwt(
    claims: &udex_api::authz::claims::Claims,
    signing_key: &EncodingKey,
) -> String {
    let mut header = Header::new(jsonwebtoken::Algorithm::ES256);
    header.typ = Some("JWT".to_string());

    encode(&header, claims, signing_key).expect("Failed to generate JWT")
}

/// Tests that the standard gRPC health service responds SERVING over TLS.
#[tokio_shared_rt::test] //We use tokio shared runtime to ensure the static variables are still valid between tests -https://docs.rs/tokio-shared-rt/latest/tokio_shared_rt/
async fn test_server_health_check() {
    // Initialize rustls crypto provider
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let data = data(false).await;
    let bind_address = data.1;

    // Load CA certificate for TLS verification
    let ca_cert = tokio::fs::read_to_string("tests/certs/ca.crt")
        .await
        .expect("Failed to read CA certificate");

    // Configure TLS for the client
    let tls_config = ClientTlsConfig::new()
        .ca_certificate(tonic::transport::Certificate::from_pem(ca_cert))
        .domain_name("localhost"); // Must match CN in server certificate

    // Create HTTPS endpoint
    let endpoint = Channel::from_shared(format!("https://{}", bind_address))
        .expect("Invalid endpoint")
        .tls_config(tls_config)
        .expect("Failed to configure TLS");

    // Connect to the standard gRPC health service with TLS
    let channel = endpoint
        .connect()
        .await
        .expect("Failed to connect to health service over TLS");
    let mut client = HealthClient::new(channel);

    // Make health check request
    let request = tonic::Request::new(HealthCheckRequest {
        service: "".to_string(),
    });
    let response = client
        .check(request)
        .await
        .expect("Health check request failed");
    let health_response = response.into_inner();

    // Verify the response
    assert_eq!(
        health_response.status,
        ServingStatus::Serving as i32,
        "Health service returned non-serving status"
    );

    // Verify per-service statuses — these are set by each service's init() and must
    // be SERVING by the time the server is accepting connections.
    for service in &["udex.entry.v1.EntryService", "udex.index.v1.IndexService"] {
        let resp = client
            .check(tonic::Request::new(HealthCheckRequest {
                service: service.to_string(),
            }))
            .await
            .unwrap_or_else(|e| panic!("Health check for {service} failed: {e}"));
        assert_eq!(
            resp.into_inner().status,
            ServingStatus::Serving as i32,
            "{service} health status is not SERVING"
        );
    }

    // Verify that an unregistered service name returns NOT_FOUND, not NOT_SERVING.
    // This ensures that registering named services as NOT_SERVING during startup
    // does not accidentally cause unknown services to appear registered.
    let unknown = client
        .check(tonic::Request::new(HealthCheckRequest {
            service: "udex.unknown.v1.UnknownService".to_string(),
        }))
        .await;
    assert!(
        matches!(&unknown, Err(s) if s.code() == tonic::Code::NotFound),
        "unregistered service should return NOT_FOUND, got: {unknown:?}"
    );

    println!("✓ Successfully connected to health service over TLS");
    println!("✓ Server is using certificate from tests/certs/server.crt");
    println!("✓ Client verified server certificate with CA from tests/certs/ca.crt");

    // Verify that non-TLS HTTP connection fails (TLS-only server)
    println!("Testing HTTP connection to verify TLS-only enforcement...");
    let http_endpoint =
        Channel::from_shared(format!("http://{}", bind_address)).expect("Invalid HTTP endpoint");

    match http_endpoint.connect().await {
        Ok(ch) => {
            // If connection succeeded, the request should still fail due to protocol mismatch
            println!("⚠ HTTP connection succeeded (server may accept both protocols)");
            let http_request = tonic::Request::new(HealthCheckRequest {
                service: "".to_string(),
            });
            let http_response_result = HealthClient::new(ch).check(http_request).await;
            assert!(
                http_response_result.is_err(),
                "HTTP request should fail on TLS-configured server"
            );
            println!("✓ HTTP request correctly failed - server enforces proper TLS protocol");
        }
        Err(e) => {
            println!(
                "✓ HTTP connection correctly failed - server enforces TLS-only: {}",
                e
            );
        }
    }
}

/// Tests that the init indexes are properly created during server startup.
#[tokio_shared_rt::test]
async fn test_server_init_indexes() {
    // Initialize rustls crypto provider
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let data = data(false).await;
    let bind_address = data.1;
    let index_name = &data.0;
    let jwt_signing_key = &data.2;

    // Load CA certificate for TLS verification
    let ca_cert = tokio::fs::read_to_string("tests/certs/ca.crt")
        .await
        .expect("Failed to read CA certificate");

    // Configure TLS for the client
    let tls_config = ClientTlsConfig::new()
        .ca_certificate(tonic::transport::Certificate::from_pem(ca_cert))
        .domain_name("localhost");

    // Create HTTPS endpoint
    let endpoint = Channel::from_shared(format!("https://{}", bind_address))
        .expect("Invalid endpoint")
        .tls_config(tls_config)
        .expect("Failed to configure TLS");

    // Connect to the index service
    let mut client = udex_api::index::index_service_client::IndexServiceClient::connect(endpoint)
        .await
        .expect("Failed to connect to index service over TLS");

    // Test that the init index exists by describing it
    let mut describe_request = tonic::Request::new(udex_api::index::DescribeRequest {
        name: index_name.clone(),
    });

    // Generate JWT token for authentication
    let claims = generate_test_claims(
        "test-user",
        &data.6,
        &data.7,
        Some(OverrideClaims {
            scope: Some(format!("udex:index:v1:{}:read", index_name)),
            sub: None,
            issuer: None,
            audience: None,
            exp: None,
            iat: None,
        }),
    );
    let jwt_token = generate_test_jwt(&claims, jwt_signing_key);
    let bearer_token = format!("Bearer {}", jwt_token);
    describe_request.metadata_mut().insert(
        "authorization",
        bearer_token.parse().expect("Failed to parse bearer token"),
    );

    let describe_response = client
        .describe(describe_request)
        .await
        .expect("Describe request failed");
    let index_response = describe_response.into_inner();

    // Verify the index was created with the expected properties
    assert!(index_response.index.is_some(), "Index should exist");
    let index = index_response.index.unwrap();

    assert_eq!(index.name, index_name.clone(), "Index name should match");
    assert_eq!(
        index.description,
        index_name.clone(),
        "Index description should match"
    );
    assert_eq!(
        index.max_bulk_operations, 100,
        "Max bulk operations should match"
    );
    assert_eq!(index.max_key_length, 256, "Max key length should match");
    assert_eq!(
        index.max_value_length, 1024,
        "Max value length should match"
    );
    assert_eq!(
        index.max_kv_pairs_per_context, 50,
        "Max KV pairs per context should match"
    );
    assert_eq!(
        index.hash_algorithm,
        HashAlgorithm::Xxh3 as i32,
        "Hash algorithm should match"
    );
    assert!(
        index.created_at.is_some(),
        "Created at timestamp should be present"
    );
    assert_eq!(index.created_by, "init", "Created by should be 'init'");

    println!("✓ Init index 'test_index' was successfully created and verified");
    println!("✓ Index properties match the configuration");
}

/// Verifies that `create_index` records the JWT `sub` claim as `created_by`.
#[tokio_shared_rt::test]
async fn test_server_create_index_records_sub_as_created_by() {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let data = data(false).await;
    let bind_address = data.1;
    let jwt_signing_key = &data.2;
    let jwt_issuer = &data.6;
    let jwt_audience = &data.7;

    let ca_cert = tokio::fs::read_to_string("tests/certs/ca.crt")
        .await
        .expect("Failed to read CA certificate");
    let tls_config = ClientTlsConfig::new()
        .ca_certificate(tonic::transport::Certificate::from_pem(ca_cert))
        .domain_name("localhost");
    let endpoint = Channel::from_shared(format!("https://{}", bind_address))
        .expect("Invalid endpoint")
        .tls_config(tls_config)
        .expect("Failed to configure TLS");

    let creator_sub = "alice";
    let new_index_name = "sub-created-by-test-index";

    let claims = generate_test_claims(
        creator_sub,
        jwt_issuer,
        jwt_audience,
        Some(OverrideClaims {
            scope: Some("udex:index:v1:create".to_string()),
            sub: None,
            issuer: None,
            audience: None,
            exp: None,
            iat: None,
        }),
    );
    let bearer = format!("Bearer {}", generate_test_jwt(&claims, jwt_signing_key));

    let mut client = udex_api::index::index_service_client::IndexServiceClient::connect(endpoint)
        .await
        .expect("Failed to connect");

    let mut create_req = tonic::Request::new(udex_api::index::CreateIndexRequest {
        name: new_index_name.to_string(),
        display_name: "Test Index Created By".to_string(),
        description: "test index for created_by verification".to_string(),
        max_bulk_operations: 10,
        max_key_length: 128,
        max_value_length: 512,
        max_kv_pairs_per_context: 16,
        hash_algorithm: udex_api::index::HashAlgorithm::Xxh3 as i32,
    });
    create_req
        .metadata_mut()
        .insert("authorization", bearer.parse().expect("valid bearer"));
    client
        .create_index(create_req)
        .await
        .expect("create_index failed");

    let read_claims = generate_test_claims(
        "reader",
        jwt_issuer,
        jwt_audience,
        Some(OverrideClaims {
            scope: Some(format!("udex:index:v1:{new_index_name}:read")),
            sub: None,
            issuer: None,
            audience: None,
            exp: None,
            iat: None,
        }),
    );
    let read_bearer = format!(
        "Bearer {}",
        generate_test_jwt(&read_claims, jwt_signing_key)
    );
    let mut describe_req = tonic::Request::new(udex_api::index::DescribeRequest {
        name: new_index_name.to_string(),
    });
    describe_req
        .metadata_mut()
        .insert("authorization", read_bearer.parse().expect("valid bearer"));
    let index = client
        .describe(describe_req)
        .await
        .expect("describe failed")
        .into_inner()
        .index
        .expect("index missing from response");

    assert_eq!(
        index.created_by, creator_sub,
        "created_by should equal the JWT sub claim"
    );
}

/// Tests authentication and authorization for different services
#[tokio_shared_rt::test]
async fn test_server_authz() {
    // Initialize rustls crypto provider
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let data = data(false).await;
    let bind_address = data.1;
    let index_name = &data.0;
    let jwt_signing_key = &data.2;
    let bad_signing_key = &data.3;

    // Load CA certificate for TLS verification
    let ca_cert = tokio::fs::read_to_string("tests/certs/ca.crt")
        .await
        .expect("Failed to read CA certificate");

    // Configure TLS for the client
    let tls_config = ClientTlsConfig::new()
        .ca_certificate(tonic::transport::Certificate::from_pem(ca_cert))
        .domain_name("localhost");

    // Create HTTPS endpoint
    let endpoint = Channel::from_shared(format!("https://{}", bind_address))
        .expect("Invalid endpoint")
        .tls_config(tls_config)
        .expect("Failed to configure TLS");

    // Test 1: Health service should work WITHOUT bearer token
    {
        let channel = endpoint
            .clone()
            .connect()
            .await
            .expect("Failed to connect to health service");
        let mut health_client = HealthClient::new(channel);

        let health_request = tonic::Request::new(HealthCheckRequest {
            service: "".to_string(),
        });
        let health_response = health_client.check(health_request).await;

        assert!(
            health_response.is_ok(),
            "Health service should work without authentication"
        );
        println!("✓ Health service works without bearer token");
    }

    // Test 2: Index service should FAIL without bearer token
    {
        let mut index_client =
            udex_api::index::index_service_client::IndexServiceClient::connect(endpoint.clone())
                .await
                .expect("Failed to connect to index service");

        let describe_request = tonic::Request::new(udex_api::index::DescribeRequest {
            name: index_name.clone(),
        });

        let describe_response = index_client.describe(describe_request).await;
        assert!(
            describe_response.is_err(),
            "Index service should fail without authentication"
        );

        let error = describe_response.unwrap_err();
        assert_eq!(
            error.code(),
            tonic::Code::Unauthenticated,
            "Should return unauthenticated error"
        );
        println!("✓ Index service correctly fails without bearer token");
    }

    // Test 3: Entry service should FAIL without bearer token
    {
        let mut entry_client =
            udex_api::entry::entry_service_client::EntryServiceClient::connect(endpoint.clone())
                .await
                .expect("Failed to connect to entry service");

        let create_request = tonic::Request::new(udex_api::entry::CreateEntryRequest {
            index_name: index_name.clone(),
            context: Some(udex_api::entry::ContextInput {
                pairs: vec![udex_api::entry::KeyValuePair {
                    key: "test_key".to_string(),
                    value: Some(udex_api::entry::Value {
                        value: Some(udex_api::entry::value::Value::StringValue(
                            "test_value".to_string(),
                        )),
                    }),
                    kek_id: None,
                    dek: None,
                }],
            }),
        });

        let create_response = entry_client.create_entry(create_request).await;
        assert!(
            create_response.is_err(),
            "Entry service should fail without authentication"
        );

        let error = create_response.unwrap_err();
        assert_eq!(
            error.code(),
            tonic::Code::Unauthenticated,
            "Should return unauthenticated error"
        );
        println!("✓ Entry service correctly fails without bearer token");
    }

    // Test 4: Index service should WORK with valid bearer token
    {
        let mut index_client =
            udex_api::index::index_service_client::IndexServiceClient::connect(endpoint.clone())
                .await
                .expect("Failed to connect to index service");

        let mut describe_request = tonic::Request::new(udex_api::index::DescribeRequest {
            name: index_name.clone(),
        });

        // Generate JWT token for authentication with proper permissions
        let claims = generate_test_claims(
            "test-user",
            &data.6,
            &data.7,
            Some(OverrideClaims {
                scope: Some(format!("udex:index:v1:{}:read", index_name)),
                sub: None,
                issuer: None,
                audience: None,
                exp: None,
                iat: None,
            }),
        );
        let jwt_token = generate_test_jwt(&claims, jwt_signing_key);
        let bearer_token = format!("Bearer {}", jwt_token);
        describe_request.metadata_mut().insert(
            "authorization",
            bearer_token.parse().expect("Failed to parse bearer token"),
        );

        let describe_response = index_client.describe(describe_request).await;
        assert!(
            describe_response.is_ok(),
            "Index service should work with valid bearer token and permissions"
        );
        println!("✓ Index service works with valid bearer token and permissions");
    }

    // Test 5: Entry service should WORK with valid bearer token
    {
        let mut entry_client =
            udex_api::entry::entry_service_client::EntryServiceClient::connect(endpoint.clone())
                .await
                .expect("Failed to connect to entry service");

        let mut create_request = tonic::Request::new(udex_api::entry::CreateEntryRequest {
            index_name: index_name.clone(),
            context: Some(udex_api::entry::ContextInput {
                pairs: vec![udex_api::entry::KeyValuePair {
                    key: "test_key".to_string(),
                    value: Some(udex_api::entry::Value {
                        value: Some(udex_api::entry::value::Value::StringValue(
                            "test_value".to_string(),
                        )),
                    }),
                    kek_id: None,
                    dek: None,
                }],
            }),
        });

        // Generate JWT token for authentication with proper permissions
        let claims = generate_test_claims(
            "test-user",
            &data.6,
            &data.7,
            Some(OverrideClaims {
                scope: Some(format!("udex:entry:v1:{}:create", index_name)),
                sub: None,
                issuer: None,
                audience: None,
                exp: None,
                iat: None,
            }),
        );
        let jwt_token = generate_test_jwt(&claims, jwt_signing_key);
        let bearer_token = format!("Bearer {}", jwt_token);
        create_request.metadata_mut().insert(
            "authorization",
            bearer_token.parse().expect("Failed to parse bearer token"),
        );

        let create_response = entry_client.create_entry(create_request).await;
        assert!(
            create_response.is_ok(),
            "Entry service should work with valid bearer token and permissions"
        );
        println!("✓ Entry service works with valid bearer token and permissions");
    }

    // Test 6: Services should FAIL with invalid bearer token
    {
        let mut index_client =
            udex_api::index::index_service_client::IndexServiceClient::connect(endpoint.clone())
                .await
                .expect("Failed to connect to index service");

        let mut describe_request = tonic::Request::new(udex_api::index::DescribeRequest {
            name: index_name.clone(),
        });

        // Generate invalid JWT token using bad signing key
        let claims = generate_test_claims("test-user", &data.6, &data.7, None);
        let invalid_jwt_token = generate_test_jwt(&claims, bad_signing_key);
        let bearer_token = format!("Bearer {}", invalid_jwt_token);
        describe_request.metadata_mut().insert(
            "authorization",
            bearer_token.parse().expect("Failed to parse bearer token"),
        );

        let describe_response = index_client.describe(describe_request).await;
        assert!(
            describe_response.is_err(),
            "Index service should fail with invalid bearer token"
        );

        let error = describe_response.unwrap_err();
        assert_eq!(
            error.code(),
            tonic::Code::Unauthenticated,
            "Should return unauthenticated error"
        );
        println!("✓ Index service correctly fails with invalid bearer token");
    }

    // Test 7: Services should FAIL with malformed authorization header
    {
        let mut entry_client =
            udex_api::entry::entry_service_client::EntryServiceClient::connect(endpoint.clone())
                .await
                .expect("Failed to connect to entry service");

        let mut create_request = tonic::Request::new(udex_api::entry::CreateEntryRequest {
            index_name: index_name.clone(),
            context: Some(udex_api::entry::ContextInput {
                pairs: vec![udex_api::entry::KeyValuePair {
                    key: "test_key".to_string(),
                    value: Some(udex_api::entry::Value {
                        value: Some(udex_api::entry::value::Value::StringValue(
                            "test_value".to_string(),
                        )),
                    }),
                    kek_id: None,
                    dek: None,
                }],
            }),
        });

        // Add malformed authorization header (missing "Bearer" prefix)
        let claims = generate_test_claims("test-user", &data.6, &data.7, None);
        let jwt_token = generate_test_jwt(&claims, jwt_signing_key);
        create_request.metadata_mut().insert(
            "authorization",
            jwt_token.parse().expect("Failed to parse jwt token"),
        );

        let create_response = entry_client.create_entry(create_request).await;
        assert!(
            create_response.is_err(),
            "Entry service should fail with malformed bearer token"
        );

        let error = create_response.unwrap_err();
        assert_eq!(
            error.code(),
            tonic::Code::Unauthenticated,
            "Should return unauthenticated error"
        );
        println!("✓ Entry service correctly fails with malformed authorization header");
    }

    // Test 8: Services should FAIL with JWT token containing empty subject
    {
        let mut index_client =
            udex_api::index::index_service_client::IndexServiceClient::connect(endpoint.clone())
                .await
                .expect("Failed to connect to index service");

        let mut describe_request = tonic::Request::new(udex_api::index::DescribeRequest {
            name: index_name.clone(),
        });

        // Generate JWT token with empty subject but all other claims valid
        let now = OffsetDateTime::now_utc().unix_timestamp() as usize;
        let claims = generate_test_claims(
            "test-user",
            &data.6,
            &data.7,
            Some(OverrideClaims {
                sub: Some("".to_string()),      // Invalid: empty subject
                issuer: Some(data.6.clone()),   // Valid
                audience: Some(data.7.clone()), // Valid
                exp: Some(now + 3600),          // Valid: expires in 1 hour
                iat: Some(now),                 // Valid: issued now
                scope: Some(format!("udex:index:v1:{}:read", index_name)),
            }),
        );
        let jwt_token = generate_test_jwt(&claims, jwt_signing_key);
        let bearer_token = format!("Bearer {}", jwt_token);
        describe_request.metadata_mut().insert(
            "authorization",
            bearer_token.parse().expect("Failed to parse bearer token"),
        );

        let describe_response = index_client.describe(describe_request).await;
        assert!(
            describe_response.is_err(),
            "Index service should fail with empty subject claim"
        );

        let error = describe_response.unwrap_err();
        assert_eq!(
            error.code(),
            tonic::Code::Unauthenticated,
            "Should return unauthenticated error"
        );
        println!("✓ Index service correctly fails with empty subject claim");
    }

    // Test 9: Services should FAIL with JWT token containing empty issuer
    {
        let mut entry_client =
            udex_api::entry::entry_service_client::EntryServiceClient::connect(endpoint.clone())
                .await
                .expect("Failed to connect to entry service");

        let mut create_request = tonic::Request::new(udex_api::entry::CreateEntryRequest {
            index_name: index_name.clone(),
            context: Some(udex_api::entry::ContextInput {
                pairs: vec![udex_api::entry::KeyValuePair {
                    key: "test_key".to_string(),
                    value: Some(udex_api::entry::Value {
                        value: Some(udex_api::entry::value::Value::StringValue(
                            "test_value".to_string(),
                        )),
                    }),
                    kek_id: None,
                    dek: None,
                }],
            }),
        });

        // Generate JWT token with empty issuer but all other claims valid
        let now = OffsetDateTime::now_utc().unix_timestamp() as usize;
        let claims = generate_test_claims(
            "test-user",
            &data.6,
            &data.7,
            Some(OverrideClaims {
                sub: Some("test-user".to_string()), // Valid
                issuer: Some("".to_string()),       // Invalid: empty issuer
                audience: Some(data.7.clone()),     // Valid
                exp: Some(now + 3600),              // Valid: expires in 1 hour
                iat: Some(now),                     // Valid: issued now
                scope: Some(format!("udex:entry:v1:{}:create", index_name)),
            }),
        );
        let jwt_token = generate_test_jwt(&claims, jwt_signing_key);
        let bearer_token = format!("Bearer {}", jwt_token);
        create_request.metadata_mut().insert(
            "authorization",
            bearer_token.parse().expect("Failed to parse bearer token"),
        );

        let create_response = entry_client.create_entry(create_request).await;
        assert!(
            create_response.is_err(),
            "Entry service should fail with empty issuer claim"
        );

        let error = create_response.unwrap_err();
        assert_eq!(
            error.code(),
            tonic::Code::Unauthenticated,
            "Should return unauthenticated error"
        );
        println!("✓ Entry service correctly fails with empty issuer claim");
    }

    // Test 10: Services should FAIL with JWT token containing empty audience
    {
        let mut index_client =
            udex_api::index::index_service_client::IndexServiceClient::connect(endpoint.clone())
                .await
                .expect("Failed to connect to index service");

        let mut describe_request = tonic::Request::new(udex_api::index::DescribeRequest {
            name: index_name.clone(),
        });

        // Generate JWT token with empty audience but all other claims valid
        let now = OffsetDateTime::now_utc().unix_timestamp() as usize;
        let claims = generate_test_claims(
            "test-user",
            &data.6,
            &data.7,
            Some(OverrideClaims {
                sub: Some("test-user".to_string()), // Valid
                issuer: Some(data.6.clone()),       // Valid
                audience: Some("".to_string()),     // Invalid: empty audience
                exp: Some(now + 3600),              // Valid: expires in 1 hour
                iat: Some(now),                     // Valid: issued now
                scope: Some(format!("udex:index:v1:{}:read", index_name)),
            }),
        );
        let jwt_token = generate_test_jwt(&claims, jwt_signing_key);
        let bearer_token = format!("Bearer {}", jwt_token);
        describe_request.metadata_mut().insert(
            "authorization",
            bearer_token.parse().expect("Failed to parse bearer token"),
        );

        let describe_response = index_client.describe(describe_request).await;
        assert!(
            describe_response.is_err(),
            "Index service should fail with empty audience claim"
        );

        let error = describe_response.unwrap_err();
        assert_eq!(
            error.code(),
            tonic::Code::Unauthenticated,
            "Should return unauthenticated error"
        );
        println!("✓ Index service correctly fails with empty audience claim");
    }

    // Test 11: Services should FAIL with JWT token containing zero expiration
    {
        let mut entry_client =
            udex_api::entry::entry_service_client::EntryServiceClient::connect(endpoint.clone())
                .await
                .expect("Failed to connect to entry service");

        let mut create_request = tonic::Request::new(udex_api::entry::CreateEntryRequest {
            index_name: index_name.clone(),
            context: Some(udex_api::entry::ContextInput {
                pairs: vec![udex_api::entry::KeyValuePair {
                    key: "test_key".to_string(),
                    value: Some(udex_api::entry::Value {
                        value: Some(udex_api::entry::value::Value::StringValue(
                            "test_value".to_string(),
                        )),
                    }),
                    kek_id: None,
                    dek: None,
                }],
            }),
        });

        // Generate JWT token with zero expiration but all other claims valid
        let now = OffsetDateTime::now_utc().unix_timestamp() as usize;
        let claims = generate_test_claims(
            "test-user",
            &data.6,
            &data.7,
            Some(OverrideClaims {
                sub: Some("test-user".to_string()), // Valid
                issuer: Some(data.6.clone()),       // Valid
                audience: Some(data.7.clone()),     // Valid
                exp: Some(0),                       // Invalid: zero expiration
                iat: Some(now),                     // Valid: issued now
                scope: Some(format!("udex:entry:v1:{}:create", index_name)),
            }),
        );
        let jwt_token = generate_test_jwt(&claims, jwt_signing_key);
        let bearer_token = format!("Bearer {}", jwt_token);
        create_request.metadata_mut().insert(
            "authorization",
            bearer_token.parse().expect("Failed to parse bearer token"),
        );

        let create_response = entry_client.create_entry(create_request).await;
        assert!(
            create_response.is_err(),
            "Entry service should fail with zero expiration claim"
        );

        let error = create_response.unwrap_err();
        assert_eq!(
            error.code(),
            tonic::Code::Unauthenticated,
            "Should return unauthenticated error"
        );
        println!("✓ Entry service correctly fails with zero expiration claim");
    }

    // Test 12: Services should FAIL with JWT token containing zero issued at
    {
        let mut index_client =
            udex_api::index::index_service_client::IndexServiceClient::connect(endpoint.clone())
                .await
                .expect("Failed to connect to index service");

        let mut describe_request = tonic::Request::new(udex_api::index::DescribeRequest {
            name: index_name.clone(),
        });

        // Generate JWT token with zero issued at but all other claims valid
        let now = OffsetDateTime::now_utc().unix_timestamp() as usize;
        let claims = generate_test_claims(
            "test-user",
            &data.6,
            &data.7,
            Some(OverrideClaims {
                sub: Some("test-user".to_string()), // Valid
                issuer: Some(data.6.clone()),       // Valid
                audience: Some(data.7.clone()),     // Valid
                exp: Some(now + 3600),              // Valid: expires in 1 hour
                iat: Some(0),                       // Invalid: zero issued at
                scope: Some(format!("udex:index:v1:{}:read", index_name)),
            }),
        );
        let jwt_token = generate_test_jwt(&claims, jwt_signing_key);
        let bearer_token = format!("Bearer {}", jwt_token);
        describe_request.metadata_mut().insert(
            "authorization",
            bearer_token.parse().expect("Failed to parse bearer token"),
        );

        let describe_response = index_client.describe(describe_request).await;
        assert!(
            describe_response.is_err(),
            "Index service should fail with zero issued at claim"
        );

        let error = describe_response.unwrap_err();
        assert_eq!(
            error.code(),
            tonic::Code::Unauthenticated,
            "Should return unauthenticated error"
        );
        println!("✓ Index service correctly fails with zero issued at claim");
    }

    // Test 13: Services should FAIL with expired JWT token
    {
        let mut index_client =
            udex_api::index::index_service_client::IndexServiceClient::connect(endpoint.clone())
                .await
                .expect("Failed to connect to index service");

        let mut describe_request = tonic::Request::new(udex_api::index::DescribeRequest {
            name: index_name.clone(),
        });

        // Generate JWT token that expired 1 hour ago
        let now = OffsetDateTime::now_utc().unix_timestamp() as usize;
        let claims = generate_test_claims(
            "test-user",
            &data.6,
            &data.7,
            Some(OverrideClaims {
                sub: Some("test-user".to_string()), // Valid
                issuer: Some(data.6.clone()),       // Valid
                audience: Some(data.7.clone()),     // Valid
                exp: Some(now - 3600),              // Invalid: expired 1 hour ago
                iat: Some(now - 7200),              // Valid: issued 2 hours ago (before expiration)
                scope: Some(format!("udex:index:v1:{}:read", index_name)),
            }),
        );
        let jwt_token = generate_test_jwt(&claims, jwt_signing_key);
        let bearer_token = format!("Bearer {}", jwt_token);
        describe_request.metadata_mut().insert(
            "authorization",
            bearer_token.parse().expect("Failed to parse bearer token"),
        );

        let describe_response = index_client.describe(describe_request).await;
        assert!(
            describe_response.is_err(),
            "Index service should fail with expired token"
        );

        let error = describe_response.unwrap_err();
        assert_eq!(
            error.code(),
            tonic::Code::Unauthenticated,
            "Should return unauthenticated error"
        );
        println!("✓ Index service correctly fails with expired token");
    }

    // Test 14: Services should FAIL with JWT token issued in the future
    {
        let mut entry_client =
            udex_api::entry::entry_service_client::EntryServiceClient::connect(endpoint.clone())
                .await
                .expect("Failed to connect to entry service");

        let mut create_request = tonic::Request::new(udex_api::entry::CreateEntryRequest {
            index_name: index_name.clone(),
            context: Some(udex_api::entry::ContextInput {
                pairs: vec![udex_api::entry::KeyValuePair {
                    key: "test_key".to_string(),
                    value: Some(udex_api::entry::Value {
                        value: Some(udex_api::entry::value::Value::StringValue(
                            "test_value".to_string(),
                        )),
                    }),
                    kek_id: None,
                    dek: None,
                }],
            }),
        });

        // Generate JWT token issued in the future with proper permissions
        let now = OffsetDateTime::now_utc().unix_timestamp() as usize;
        let claims = generate_test_claims(
            "test-user",
            &data.6,
            &data.7,
            Some(OverrideClaims {
                sub: None,
                issuer: None,
                audience: None,
                exp: Some(now + 7200), // expires 2 hours from now
                iat: Some(now + 3600), // issued 1 hour in the future
                scope: Some(format!("udex:entry:v1:{}:create", index_name)),
            }),
        );
        let jwt_token = generate_test_jwt(&claims, jwt_signing_key);
        let bearer_token = format!("Bearer {}", jwt_token);
        create_request.metadata_mut().insert(
            "authorization",
            bearer_token.parse().expect("Failed to parse bearer token"),
        );

        let create_response = entry_client.create_entry(create_request).await;
        // Note: JWT library doesn't validate future iat claims, so this currently passes
        // This test demonstrates that future issued tokens are NOT rejected by default
        assert!(create_response.is_ok(), "Entry service currently accepts future issued tokens (JWT library doesn't validate iat)");
        println!("⚠️  Entry service currently accepts future issued token (timestamp validation not enabled)");
    }

    // Test 15: Services should FAIL with JWT token where iat > exp (invalid time range)
    {
        let mut index_client =
            udex_api::index::index_service_client::IndexServiceClient::connect(endpoint.clone())
                .await
                .expect("Failed to connect to index service");

        let mut describe_request = tonic::Request::new(udex_api::index::DescribeRequest {
            name: index_name.clone(),
        });

        // Generate JWT token where issued at is after expiration (invalid)
        let now = OffsetDateTime::now_utc().unix_timestamp() as usize;
        let claims = generate_test_claims(
            "test-user",
            &data.6,
            &data.7,
            Some(OverrideClaims {
                sub: Some("test-user".to_string()), // Valid
                issuer: Some(data.6.clone()),       // Valid
                audience: Some(data.7.clone()),     // Valid
                exp: Some(now - 1800),              // Invalid: expires 30 minutes ago (in past)
                iat: Some(now - 900), // Invalid: issued 15 minutes ago (after expiration) - creates impossible timeline
                scope: Some(format!("udex:index:v1:{}:read", index_name)),
            }),
        );
        let jwt_token = generate_test_jwt(&claims, jwt_signing_key);
        let bearer_token = format!("Bearer {}", jwt_token);
        describe_request.metadata_mut().insert(
            "authorization",
            bearer_token.parse().expect("Failed to parse bearer token"),
        );

        let describe_response = index_client.describe(describe_request).await;
        // Note: This token is expired (exp in the past), so JWT library rejects it
        assert!(
            describe_response.is_err(),
            "Index service should fail with expired token (regardless of iat)"
        );

        let error = describe_response.unwrap_err();
        assert_eq!(
            error.code(),
            tonic::Code::Unauthenticated,
            "Should return unauthenticated error"
        );
        println!("✓ Index service correctly fails with expired token (iat validation not needed)");
    }

    // Test 16: Index service should FAIL with wrong permissions
    {
        let mut index_client =
            udex_api::index::index_service_client::IndexServiceClient::connect(endpoint.clone())
                .await
                .expect("Failed to connect to index service");

        let mut describe_request = tonic::Request::new(udex_api::index::DescribeRequest {
            name: index_name.clone(),
        });

        // Generate JWT token with wrong permission (entry permission for index service)
        let claims = generate_test_claims(
            "test-user",
            &data.6,
            &data.7,
            Some(OverrideClaims {
                scope: Some("udex:entry:v1:read".to_string()),
                sub: None,
                issuer: None,
                audience: None,
                exp: None,
                iat: None,
            }),
        );
        let jwt_token = generate_test_jwt(&claims, jwt_signing_key);
        let bearer_token = format!("Bearer {}", jwt_token);
        describe_request.metadata_mut().insert(
            "authorization",
            bearer_token.parse().expect("Failed to parse bearer token"),
        );

        let describe_response = index_client.describe(describe_request).await;
        assert!(
            describe_response.is_err(),
            "Index service should fail with wrong permissions"
        );

        let error = describe_response.unwrap_err();
        assert_eq!(
            error.code(),
            tonic::Code::PermissionDenied,
            "Should return permission denied error"
        );
        println!("✓ Index service correctly fails with wrong permissions");
    }

    // Test 17: Entry service should FAIL with wrong permissions
    {
        let mut entry_client =
            udex_api::entry::entry_service_client::EntryServiceClient::connect(endpoint.clone())
                .await
                .expect("Failed to connect to entry service");

        let mut create_request = tonic::Request::new(udex_api::entry::CreateEntryRequest {
            index_name: index_name.clone(),
            context: Some(udex_api::entry::ContextInput {
                pairs: vec![udex_api::entry::KeyValuePair {
                    key: "test_key".to_string(),
                    value: Some(udex_api::entry::Value {
                        value: Some(udex_api::entry::value::Value::StringValue(
                            "test_value".to_string(),
                        )),
                    }),
                    kek_id: None,
                    dek: None,
                }],
            }),
        });

        // Generate JWT token with wrong permission (index permission for entry service)
        let claims = generate_test_claims(
            "test-user",
            &data.6,
            &data.7,
            Some(OverrideClaims {
                scope: Some("udex:index:v1:read".to_string()),
                sub: None,
                issuer: None,
                audience: None,
                exp: None,
                iat: None,
            }),
        );
        let jwt_token = generate_test_jwt(&claims, jwt_signing_key);
        let bearer_token = format!("Bearer {}", jwt_token);
        create_request.metadata_mut().insert(
            "authorization",
            bearer_token.parse().expect("Failed to parse bearer token"),
        );

        let create_response = entry_client.create_entry(create_request).await;
        assert!(
            create_response.is_err(),
            "Entry service should fail with wrong permissions"
        );

        let error = create_response.unwrap_err();
        assert_eq!(
            error.code(),
            tonic::Code::PermissionDenied,
            "Should return permission denied error"
        );
        println!("✓ Entry service correctly fails with wrong permissions");
    }

    // Test 18: Services should FAIL with no permissions field in claims
    {
        let mut index_client =
            udex_api::index::index_service_client::IndexServiceClient::connect(endpoint.clone())
                .await
                .expect("Failed to connect to index service");

        let mut describe_request = tonic::Request::new(udex_api::index::DescribeRequest {
            name: index_name.clone(),
        });

        // Generate JWT token without permissions field
        let claims = generate_test_claims("test-user", &data.6, &data.7, None);
        let jwt_token = generate_test_jwt(&claims, jwt_signing_key);
        let bearer_token = format!("Bearer {}", jwt_token);
        describe_request.metadata_mut().insert(
            "authorization",
            bearer_token.parse().expect("Failed to parse bearer token"),
        );

        let describe_response = index_client.describe(describe_request).await;
        assert!(
            describe_response.is_err(),
            "Index service should fail with no permissions field"
        );

        let error = describe_response.unwrap_err();
        assert_eq!(
            error.code(),
            tonic::Code::PermissionDenied,
            "Should return permission denied error"
        );
        println!("✓ Index service correctly fails with no permissions field");
    }

    // Test 19: Services should FAIL with permissions as string instead of array
    {
        let mut entry_client =
            udex_api::entry::entry_service_client::EntryServiceClient::connect(endpoint.clone())
                .await
                .expect("Failed to connect to entry service");

        let mut create_request = tonic::Request::new(udex_api::entry::CreateEntryRequest {
            index_name: index_name.clone(),
            context: Some(udex_api::entry::ContextInput {
                pairs: vec![udex_api::entry::KeyValuePair {
                    key: "test_key".to_string(),
                    value: Some(udex_api::entry::Value {
                        value: Some(udex_api::entry::value::Value::StringValue(
                            "test_value".to_string(),
                        )),
                    }),
                    kek_id: None,
                    dek: None,
                }],
            }),
        });

        // Non-udex: scope values are silently discarded → no permissions granted → denied
        let claims = generate_test_claims(
            "test-user",
            &data.6,
            &data.7,
            Some(OverrideClaims {
                scope: Some("openid profile email".to_string()),
                sub: None,
                issuer: None,
                audience: None,
                exp: None,
                iat: None,
            }),
        );
        let jwt_token = generate_test_jwt(&claims, jwt_signing_key);
        let bearer_token = format!("Bearer {}", jwt_token);
        create_request.metadata_mut().insert(
            "authorization",
            bearer_token.parse().expect("Failed to parse bearer token"),
        );

        let create_response = entry_client.create_entry(create_request).await;
        assert!(
            create_response.is_err(),
            "Entry service should fail when scope contains only non-udex values"
        );

        let error = create_response.unwrap_err();
        assert_eq!(
            error.code(),
            tonic::Code::PermissionDenied,
            "Should return permission denied error"
        );
        println!("✓ Entry service correctly fails when scope contains only non-udex values");
    }

    // Test 19b: Services should SUCCEED when scope mixes non-udex and valid udex values
    {
        let mut entry_client =
            udex_api::entry::entry_service_client::EntryServiceClient::connect(endpoint.clone())
                .await
                .expect("Failed to connect to entry service");

        let mut create_request = tonic::Request::new(udex_api::entry::CreateEntryRequest {
            index_name: index_name.clone(),
            context: Some(udex_api::entry::ContextInput {
                pairs: vec![udex_api::entry::KeyValuePair {
                    key: "test_key".to_string(),
                    value: Some(udex_api::entry::Value {
                        value: Some(udex_api::entry::value::Value::StringValue(
                            "test_value".to_string(),
                        )),
                    }),
                    kek_id: None,
                    dek: None,
                }],
            }),
        });

        // Mixed scope: non-udex values are silently discarded; the udex: value grants access
        let claims = generate_test_claims(
            "test-user",
            &data.6,
            &data.7,
            Some(OverrideClaims {
                scope: Some(format!(
                    "openid profile email udex:entry:v1:{}:create",
                    index_name
                )),
                sub: None,
                issuer: None,
                audience: None,
                exp: None,
                iat: None,
            }),
        );
        let jwt_token = generate_test_jwt(&claims, jwt_signing_key);
        let bearer_token = format!("Bearer {}", jwt_token);
        create_request.metadata_mut().insert(
            "authorization",
            bearer_token.parse().expect("Failed to parse bearer token"),
        );

        let create_response = entry_client.create_entry(create_request).await;
        assert!(
            create_response.is_ok(),
            "Entry service should succeed when scope mixes non-udex and valid udex values"
        );
        println!(
            "✓ Entry service succeeds when scope mixes non-udex and valid udex values (non-udex silently discarded)"
        );
    }

    // Test 20: Services should FAIL with empty scope
    {
        let mut index_client =
            udex_api::index::index_service_client::IndexServiceClient::connect(endpoint.clone())
                .await
                .expect("Failed to connect to index service");

        let mut describe_request = tonic::Request::new(udex_api::index::DescribeRequest {
            name: index_name.clone(),
        });

        // Empty scope claim → no permissions granted → denied
        let claims = generate_test_claims(
            "test-user",
            &data.6,
            &data.7,
            Some(OverrideClaims {
                scope: Some("".to_string()),
                sub: None,
                issuer: None,
                audience: None,
                exp: None,
                iat: None,
            }),
        );
        let jwt_token = generate_test_jwt(&claims, jwt_signing_key);
        let bearer_token = format!("Bearer {}", jwt_token);
        describe_request.metadata_mut().insert(
            "authorization",
            bearer_token.parse().expect("Failed to parse bearer token"),
        );

        let describe_response = index_client.describe(describe_request).await;
        assert!(
            describe_response.is_err(),
            "Index service should fail with empty scope"
        );

        let error = describe_response.unwrap_err();
        assert_eq!(
            error.code(),
            tonic::Code::PermissionDenied,
            "Should return permission denied error"
        );
        println!("✓ Index service correctly fails with empty scope");
    }

    // Test 21: Services should FAIL with no scope claim
    {
        let mut entry_client =
            udex_api::entry::entry_service_client::EntryServiceClient::connect(endpoint.clone())
                .await
                .expect("Failed to connect to entry service");

        let mut create_request = tonic::Request::new(udex_api::entry::CreateEntryRequest {
            index_name: index_name.clone(),
            context: Some(udex_api::entry::ContextInput {
                pairs: vec![udex_api::entry::KeyValuePair {
                    key: "test_key".to_string(),
                    value: Some(udex_api::entry::Value {
                        value: Some(udex_api::entry::value::Value::StringValue(
                            "test_value".to_string(),
                        )),
                    }),
                    kek_id: None,
                    dek: None,
                }],
            }),
        });

        // No scope claim → no permissions → denied
        let claims = generate_test_claims("test-user", &data.6, &data.7, None);
        let jwt_token = generate_test_jwt(&claims, jwt_signing_key);
        let bearer_token = format!("Bearer {}", jwt_token);
        create_request.metadata_mut().insert(
            "authorization",
            bearer_token.parse().expect("Failed to parse bearer token"),
        );

        let create_response = entry_client.create_entry(create_request).await;
        assert!(
            create_response.is_err(),
            "Entry service should fail with no scope claim"
        );

        let error = create_response.unwrap_err();
        assert_eq!(
            error.code(),
            tonic::Code::PermissionDenied,
            "Should return permission denied error"
        );
        println!("✓ Entry service correctly fails with no scope claim");
    }

    // Test 22: Index service should work when user has multiple permissions including the required one
    {
        let mut index_client =
            udex_api::index::index_service_client::IndexServiceClient::connect(endpoint.clone())
                .await
                .expect("Failed to connect to index service");

        let mut describe_request = tonic::Request::new(udex_api::index::DescribeRequest {
            name: index_name.clone(),
        });

        // Generate JWT token with multiple permissions including the required one for describe
        let claims = generate_test_claims(
            "test-user",
            &data.6,
            &data.7,
            Some(OverrideClaims {
                scope: Some(format!(
                    "udex:index:v1:{0}:read udex:index:v1:{0}:write udex:entry:v1:{0}:create",
                    index_name
                )),
                sub: None,
                issuer: None,
                audience: None,
                exp: None,
                iat: None,
            }),
        );
        let jwt_token = generate_test_jwt(&claims, jwt_signing_key);
        let bearer_token = format!("Bearer {}", jwt_token);
        describe_request.metadata_mut().insert(
            "authorization",
            bearer_token.parse().expect("Failed to parse bearer token"),
        );

        let describe_response = index_client.describe(describe_request).await;
        assert!(
            describe_response.is_ok(),
            "Index describe should work when user has multiple permissions including required one"
        );
        println!(
            "✓ Index describe works when user has multiple permissions including required one"
        );
    }

    // Test 23: Index service should FAIL when user has permissions but not the required one
    {
        let mut index_client =
            udex_api::index::index_service_client::IndexServiceClient::connect(endpoint.clone())
                .await
                .expect("Failed to connect to index service");

        let mut describe_request = tonic::Request::new(udex_api::index::DescribeRequest {
            name: index_name.clone(),
        });

        // Generate JWT token with permissions but not the required one for describe
        let claims = generate_test_claims(
            "test-user",
            &data.6,
            &data.7,
            Some(OverrideClaims {
                // Missing the required udex:index:v1:{name}:read permission
                scope: Some("udex:index:v1:write udex:entry:v1:read".to_string()),
                sub: None,
                issuer: None,
                audience: None,
                exp: None,
                iat: None,
            }),
        );
        let jwt_token = generate_test_jwt(&claims, jwt_signing_key);
        let bearer_token = format!("Bearer {}", jwt_token);
        describe_request.metadata_mut().insert(
            "authorization",
            bearer_token.parse().expect("Failed to parse bearer token"),
        );

        let describe_response = index_client.describe(describe_request).await;
        assert!(
            describe_response.is_err(),
            "Index describe should fail when user has permissions but not the required one"
        );

        let error = describe_response.unwrap_err();
        assert_eq!(
            error.code(),
            tonic::Code::PermissionDenied,
            "Should return permission denied error"
        );
        println!(
            "✓ Index describe correctly fails when user has permissions but not the required one"
        );
    }

    println!("✓ All authentication and authorization tests passed successfully");
}

/// Verifies that a client presenting a wrong CA certificate cannot connect.
///
/// Uses the server's own leaf certificate as the "CA" — it is not a CA cert and
/// was not used to sign itself, so rustls must reject the handshake.
#[tokio_shared_rt::test]
async fn test_server_tls_wrong_ca_rejected() {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let data = data(false).await;
    let bind_address = data.1;

    // Deliberately supply the server's leaf cert as the trusted CA — this is wrong.
    let wrong_ca = tokio::fs::read_to_string("tests/certs/server.crt")
        .await
        .expect("Failed to read server cert");

    let tls_config = ClientTlsConfig::new()
        .ca_certificate(tonic::transport::Certificate::from_pem(wrong_ca))
        .domain_name("localhost");

    let endpoint = Channel::from_shared(format!("https://{}", bind_address))
        .expect("Invalid endpoint")
        .tls_config(tls_config)
        .expect("Failed to configure TLS");

    let err = endpoint
        .connect()
        .await
        .expect_err("Connection with wrong CA certificate must be rejected");
    let err_str = format!("{err:?}");
    assert!(
        err_str.to_lowercase().contains("certificate"),
        "Expected a certificate validation error, got: {err_str}"
    );
    println!("✓ Connection with wrong CA correctly rejected");
}

/// Verifies that a valid, unexpired token is accepted on two sequential requests
/// (i.e. the server has no token blacklist and does not revoke on first use).
#[tokio_shared_rt::test]
async fn test_server_token_reuse() {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let data = data(false).await;
    let bind_address = data.1;
    let index_name = &data.0;
    let jwt_signing_key = &data.2;

    let ca_cert = tokio::fs::read_to_string("tests/certs/ca.crt")
        .await
        .expect("Failed to read CA certificate");
    let tls_config = ClientTlsConfig::new()
        .ca_certificate(tonic::transport::Certificate::from_pem(ca_cert))
        .domain_name("localhost");
    let endpoint = Channel::from_shared(format!("https://{}", bind_address))
        .expect("Invalid endpoint")
        .tls_config(tls_config)
        .expect("Failed to configure TLS");

    let claims = generate_test_claims(
        "test-user",
        &data.6,
        &data.7,
        Some(OverrideClaims {
            scope: Some(format!("udex:index:v1:{index_name}:read")),
            sub: None,
            issuer: None,
            audience: None,
            exp: None,
            iat: None,
        }),
    );
    let jwt_token = generate_test_jwt(&claims, jwt_signing_key);
    let bearer_token = format!("Bearer {jwt_token}");

    let mut index_client =
        udex_api::index::index_service_client::IndexServiceClient::connect(endpoint)
            .await
            .expect("Failed to connect");

    let mut req1 = tonic::Request::new(udex_api::index::DescribeRequest {
        name: index_name.clone(),
    });
    req1.metadata_mut().insert(
        "authorization",
        bearer_token.parse().expect("Failed to parse bearer token"),
    );
    assert!(
        index_client.describe(req1).await.is_ok(),
        "First use of token should succeed"
    );

    let mut req2 = tonic::Request::new(udex_api::index::DescribeRequest {
        name: index_name.clone(),
    });
    req2.metadata_mut().insert(
        "authorization",
        bearer_token.parse().expect("Failed to parse bearer token"),
    );
    assert!(
        index_client.describe(req2).await.is_ok(),
        "Re-use of same token should succeed (no server-side token blacklist)"
    );

    println!("✓ Token re-use: same unexpired token accepted on two sequential requests");
}

/// Verifies that a Hydra-issued token is accepted twice in a row.
#[tokio_shared_rt::test]
async fn test_server_oauth2_token_reuse() {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let data = data_using_hydra(false).await;
    let bind_address = data.1;
    let index_name = &data.0;
    let client_config = data.2.clone();
    let public_url = &data.3;

    let token = auth_server::authenticate(
        public_url,
        client_config,
        vec![format!("udex:index:v1:{index_name}:read")],
    )
    .await
    .expect("Failed to obtain Hydra token");

    let ca_cert = tokio::fs::read_to_string("tests/certs/ca.crt")
        .await
        .expect("Failed to read CA certificate");
    let tls_config = ClientTlsConfig::new()
        .ca_certificate(tonic::transport::Certificate::from_pem(ca_cert))
        .domain_name("localhost");
    let endpoint = Channel::from_shared(format!("https://{}", bind_address))
        .expect("Invalid endpoint")
        .tls_config(tls_config)
        .expect("Failed to configure TLS");

    let bearer_token = format!("Bearer {token}");

    let mut index_client =
        udex_api::index::index_service_client::IndexServiceClient::connect(endpoint)
            .await
            .expect("Failed to connect");

    let mut req1 = tonic::Request::new(udex_api::index::DescribeRequest {
        name: index_name.clone(),
    });
    req1.metadata_mut().insert(
        "authorization",
        bearer_token.parse().expect("Failed to parse bearer token"),
    );
    assert!(
        index_client.describe(req1).await.is_ok(),
        "First Hydra token request should succeed"
    );

    let mut req2 = tonic::Request::new(udex_api::index::DescribeRequest {
        name: index_name.clone(),
    });
    req2.metadata_mut().insert(
        "authorization",
        bearer_token.parse().expect("Failed to parse bearer token"),
    );
    assert!(
        index_client.describe(req2).await.is_ok(),
        "Re-use of same Hydra token should succeed"
    );

    println!("✓ Hydra token re-use: same token accepted on two sequential requests");
}

/// Verifies scope sub-setting against a live Hydra instance.
///
/// A single Hydra client is registered with two scopes (index read + entry
/// create). The test authenticates requesting only the index read scope, then
/// confirms the server grants index read but denies entry create — proving the
/// token carries only the requested subset of scopes.
///
#[tokio_shared_rt::test]
async fn test_server_oauth2_scope_subset() {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let data = data_using_hydra(false).await;
    let bind_address = data.1;
    let index_name = &data.0;
    let client_config = data.2.clone();
    let public_url = &data.3;

    // Authenticate with only the index read scope — a strict subset of the
    // client's registered scopes (which also include entry create).
    let token = auth_server::authenticate(
        public_url,
        client_config,
        vec![format!("udex:index:v1:{index_name}:read")],
    )
    .await
    .expect("Failed to obtain Hydra token for subset scope");

    let ca_cert = tokio::fs::read_to_string("tests/certs/ca.crt")
        .await
        .expect("Failed to read CA certificate");
    let tls_config = ClientTlsConfig::new()
        .ca_certificate(tonic::transport::Certificate::from_pem(ca_cert))
        .domain_name("localhost");
    let endpoint = Channel::from_shared(format!("https://{}", bind_address))
        .expect("Invalid endpoint")
        .tls_config(tls_config)
        .expect("Failed to configure TLS");

    let bearer_token = format!("Bearer {token}");

    // Index describe should SUCCEED — token has udex:index:v1:{index}:read.
    {
        let mut index_client =
            udex_api::index::index_service_client::IndexServiceClient::connect(endpoint.clone())
                .await
                .expect("Failed to connect to index service");

        let mut req = tonic::Request::new(udex_api::index::DescribeRequest {
            name: index_name.clone(),
        });
        req.metadata_mut().insert(
            "authorization",
            bearer_token.parse().expect("Failed to parse bearer token"),
        );
        assert!(
            index_client.describe(req).await.is_ok(),
            "Index describe should succeed: token has the required read scope"
        );
        println!("✓ Hydra scope subset: index read scope grants describe access");
    }

    // Entry create should FAIL — token lacks udex:entry:v1:{index}:create.
    {
        let mut entry_client =
            udex_api::entry::entry_service_client::EntryServiceClient::connect(endpoint)
                .await
                .expect("Failed to connect to entry service");

        let mut req = tonic::Request::new(udex_api::entry::CreateEntryRequest {
            index_name: index_name.clone(),
            context: Some(udex_api::entry::ContextInput {
                pairs: vec![udex_api::entry::KeyValuePair {
                    key: "scope_test_key".to_string(),
                    value: Some(udex_api::entry::Value {
                        value: Some(udex_api::entry::value::Value::StringValue(
                            "scope_test_value".to_string(),
                        )),
                    }),
                    kek_id: None,
                    dek: None,
                }],
            }),
        });
        req.metadata_mut().insert(
            "authorization",
            bearer_token.parse().expect("Failed to parse bearer token"),
        );

        let response = entry_client.create_entry(req).await;
        assert!(
            response.is_err(),
            "Entry create should be denied: token does not have the entry create scope"
        );
        assert_eq!(
            response.unwrap_err().code(),
            tonic::Code::PermissionDenied,
            "Should return PermissionDenied"
        );
        println!("✓ Hydra scope subset: missing entry create scope correctly denied");
    }
}

/// Verifies that a token carrying only non-udex scopes (e.g. `api:read`) is
/// accepted as a valid JWT but denied by the permission layer because it
/// contains no `udex:*` scope values.
///
#[tokio_shared_rt::test]
async fn test_server_oauth2_non_udex_scopes_denied() {
    let admin_url = hydra_admin_url();
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let data = data_using_hydra(false).await;
    let bind_address = data.1;
    let public_url = &data.3;

    // A client registered with a non-udex scope only.
    let client = auth_server::OAuthClientConfig {
        name: format!("{ID_USING_HYDRA_PREFIX}-non-udex-client"),
        id: format!("{ID_USING_HYDRA_PREFIX}-non-udex-client"),
        secret: "non-udex-secret".to_string(),
        scopes: vec!["api:read".to_string()],
        audience: format!("{ID_USING_HYDRA_PREFIX}-audience"),
    };

    auth_server::create_oauth2_client(&admin_url, client.clone())
        .await
        .expect("Failed to create non-udex Hydra client");

    let token = auth_server::authenticate(public_url, client, vec!["api:read".to_string()])
        .await
        .expect("Failed to obtain Hydra token with non-udex scope");

    let ca_cert = tokio::fs::read_to_string("tests/certs/ca.crt")
        .await
        .expect("Failed to read CA certificate");
    let tls_config = ClientTlsConfig::new()
        .ca_certificate(tonic::transport::Certificate::from_pem(ca_cert))
        .domain_name("localhost");
    let endpoint = Channel::from_shared(format!("https://{}", bind_address))
        .expect("Invalid endpoint")
        .tls_config(tls_config)
        .expect("Failed to configure TLS");

    let mut index_client =
        udex_api::index::index_service_client::IndexServiceClient::connect(endpoint)
            .await
            .expect("Failed to connect to index service");

    let index_name = &data.0;
    let mut req = tonic::Request::new(udex_api::index::DescribeRequest {
        name: index_name.clone(),
    });
    req.metadata_mut().insert(
        "authorization",
        format!("Bearer {token}")
            .parse()
            .expect("Failed to parse bearer token"),
    );

    let response = index_client.describe(req).await;
    assert!(
        response.is_err(),
        "Request with non-udex scope token should be denied"
    );
    assert_eq!(
        response.unwrap_err().code(),
        tonic::Code::PermissionDenied,
        "Non-udex token should yield PermissionDenied, not Unauthenticated"
    );
    println!("✓ Hydra: token with non-udex scope only is correctly denied with PermissionDenied");
}

/// Verifies that a token issued to a client configured with the wrong audience
/// is rejected by the server with Unauthenticated.
///
/// Hydra embeds the client's `audience` field as the JWT `aud` claim; the
/// server is configured to expect a different audience, so validation fails.
///
/// Note: the JWT issuer is controlled globally by Hydra's URLS_SELF_ISSUER
/// and is the same for all clients on the same Hydra instance. Wrong-issuer
/// behaviour is already covered by the static-PEM test suite.
#[tokio_shared_rt::test]
async fn test_server_oauth2_wrong_audience_rejected() {
    let admin_url = hydra_admin_url();
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let data = data_using_hydra(false).await;
    let bind_address = data.1;
    let index_name = &data.0;
    let public_url = &data.3;

    // Client configured with an audience that does not match the server's
    // expected jwt_audience ("hydra-integration-test-audience").
    let client = auth_server::OAuthClientConfig {
        name: format!("{ID_USING_HYDRA_PREFIX}-wrong-aud-client"),
        id: format!("{ID_USING_HYDRA_PREFIX}-wrong-aud-client"),
        secret: "wrong-aud-secret".to_string(),
        scopes: vec![format!("udex:index:v1:{index_name}:read")],
        audience: "wrong-audience".to_string(),
    };

    auth_server::create_oauth2_client(&admin_url, client.clone())
        .await
        .expect("Failed to create wrong-audience Hydra client");

    let token = auth_server::authenticate(
        public_url,
        client,
        vec![format!("udex:index:v1:{index_name}:read")],
    )
    .await
    .expect("Failed to obtain Hydra token with wrong audience");

    let ca_cert = tokio::fs::read_to_string("tests/certs/ca.crt")
        .await
        .expect("Failed to read CA certificate");
    let tls_config = ClientTlsConfig::new()
        .ca_certificate(tonic::transport::Certificate::from_pem(ca_cert))
        .domain_name("localhost");
    let endpoint = Channel::from_shared(format!("https://{}", bind_address))
        .expect("Invalid endpoint")
        .tls_config(tls_config)
        .expect("Failed to configure TLS");

    let mut index_client =
        udex_api::index::index_service_client::IndexServiceClient::connect(endpoint)
            .await
            .expect("Failed to connect to index service");

    let mut req = tonic::Request::new(udex_api::index::DescribeRequest {
        name: index_name.clone(),
    });
    req.metadata_mut().insert(
        "authorization",
        format!("Bearer {token}")
            .parse()
            .expect("Failed to parse bearer token"),
    );

    let response = index_client.describe(req).await;
    assert!(
        response.is_err(),
        "Token with wrong audience should be rejected"
    );
    assert_eq!(
        response.unwrap_err().code(),
        tonic::Code::Unauthenticated,
        "Wrong-audience token should yield Unauthenticated"
    );
    println!("✓ Hydra: token with wrong audience is correctly rejected with Unauthenticated");
}

/// Verifies that a client relying on the system trust store cannot connect.
///
/// The test server uses a self-signed CA that is not installed in the system
/// trust store, so the TLS handshake must fail without an explicit CA override.
#[tokio_shared_rt::test]
async fn test_server_tls_untrusted_ca_rejected() {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let data = data(false).await;
    let bind_address = data.1;

    // No explicit CA — falls back to the system trust store, which does not
    // contain the self-signed test CA.
    let tls_config = ClientTlsConfig::new().domain_name("localhost");

    let endpoint = Channel::from_shared(format!("https://{}", bind_address))
        .expect("Invalid endpoint")
        .tls_config(tls_config)
        .expect("Failed to configure TLS");

    let err = endpoint
        .connect()
        .await
        .expect_err("Connection using system trust store must be rejected for self-signed test CA");
    let err_str = format!("{err:?}");
    assert!(
        err_str.to_lowercase().contains("certificate"),
        "Expected a certificate validation error, got: {err_str}"
    );
    println!("✓ Connection using system trust store correctly rejected");
}
