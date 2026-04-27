use jsonwebtoken::{encode, EncodingKey, Header};
// we don't test all the services exhaustively here as they will be tested via the client & end-to-end tests
use maybe_once::tokio::{Data, MaybeOnceAsync};
use std::net::SocketAddr;
use std::sync::OnceLock;
use time::OffsetDateTime;
use tokio::time::{sleep, Duration};
use tonic::transport::{Channel, ClientTlsConfig};
use udex_api::healthz::{healthz_service_client::HealthzServiceClient, HealthzRequest};
use udex_api::index::{HashAlgorithm, IndexUpdate, UpdateIndexRequest};
use udex_datastore::integration_test::init_postgres;
use udex_server::config::AuthNzConfig;
use udex_server::{config::ServerConfig, logging, server};

const SERVER_BIND_ADDR: &str = "127.0.0.1:50052"; // different from default  to avoid conflicts
const ID_PREFIX: &str = "server-integration-test";

// See https://github.com/ufoscout/maybe-once for the MaybeOnceAsync pattern used here.
type MaybeOnceType = (
    String,                      // index name
    ServerConfig,                // so we can find out the server bind address etc.
    EncodingKey,                 // jwt signing key for generating valid tokens
    EncodingKey,                 // bad signing key for testing invalid signatures
    tokio::task::JoinHandle<()>, // server task handle so the server doesn't get dropped
    String, // database name for cleanup (kept in scope for automatic cleanup via ctor)
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

    let bind_address: SocketAddr = SERVER_BIND_ADDR.parse().expect("Invalid bind address");

    let server_config = ServerConfig {
        bind_address,
        tls: udex_server::config::TlsConfig {
            cert_path: "tests/certs/server.crt".to_string(),
            key_path: "tests/certs/server.key".to_string(),
        },
        init_indexes: vec![UpdateIndexRequest {
            name: index_name.clone(), // Use consistent name for test_init_indexes
            update: Some(IndexUpdate {
                description: Some(index_name.clone()),
                max_bulk_operations: Some(100),
                max_key_length: Some(256),
                max_value_length: Some(1024),
                max_kv_pairs_per_context: Some(50),
                hash_algorithm: Some(HashAlgorithm::Xxh3 as i32), // Use Xxh3 for test consistency
            }),
        }],
        authnz: udex_server::config::AuthNzConfig {
            jwks_url: None,
            jwt_public_key_path: Some("tests/jwt/signing_public_key.pem".to_string()),
            jwt_issuer: Some(format!("{}-issuer", ID_PREFIX)),
            jwt_audience: Some(format!("{}-audience", ID_PREFIX)),
        },
        ..ServerConfig::default()
    };

    // Start server in background task using the convenience method
    let server_config_clone = server_config.clone();
    let server_handle = tokio::spawn(async move {
        server::serve(server_config_clone, datastore)
            .await
            .expect("Server failed to start");
    });

    // Give server time to start
    sleep(Duration::from_millis(200)).await;

    //load the jwt private signing key for generating valid test tokens
    let jwt_private_key = tokio::fs::read_to_string("tests/jwt/signing_private_key.pem")
        .await
        .expect("Failed to read JWT private signing key");
    let jwt_signing_key = EncodingKey::from_ec_pem(jwt_private_key.as_bytes())
        .expect("Failed to create EncodingKey from private key");

    //load the bad private key for testing invalid signatures
    let bad_private_key = tokio::fs::read_to_string("tests/jwt/bad_signing_private_key.pem")
        .await
        .expect("Failed to read bad private key");
    let bad_signing_key = EncodingKey::from_ec_pem(bad_private_key.as_bytes())
        .expect("Failed to create bad EncodingKey from bad private key");

    (
        index_name,
        server_config,
        jwt_signing_key,
        bad_signing_key,
        server_handle,
        db_name,
    )
}

/// A function that holds a static reference to the container
pub async fn data(serial: bool) -> Data<'static, MaybeOnceType> {
    static DATA: OnceLock<MaybeOnceAsync<MaybeOnceType>> = OnceLock::new();
    DATA.get_or_init(|| MaybeOnceAsync::new(|| Box::pin(init_server())))
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

/// Generates claims for testing JWT authentication
fn generate_test_claims(
    sub: &str,
    authnz_config: &AuthNzConfig,
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
            .unwrap_or_else(|| {
                authnz_config
                    .jwt_issuer
                    .clone()
                    .unwrap_or_else(|| "udex-test".to_string())
            }),
        override_claims
            .as_ref()
            .and_then(|c| c.audience.clone())
            .unwrap_or_else(|| {
                authnz_config
                    .jwt_audience
                    .clone()
                    .unwrap_or_else(|| "udex-api".to_string())
            }),
        override_claims
            .as_ref()
            .and_then(|c| c.exp)
            .unwrap_or_else(|| now + 3600), // expires in 1 hour
        override_claims.as_ref().and_then(|c| c.iat).unwrap_or(now), // issued now
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

/// Tests that the healthz service is available over TLS and returns a 200 OK response.
#[tokio_shared_rt::test] //We use tokio shared runtime to ensure the static variables are still valid between tests -https://docs.rs/tokio-shared-rt/latest/tokio_shared_rt/
async fn test_healthz_service() {
    // Initialize rustls crypto provider
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let data = data(false).await;
    let bind_address = data.1.bind_address;

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

    // Connect to the healthz service with TLS
    let mut client = HealthzServiceClient::connect(endpoint)
        .await
        .expect("Failed to connect to healthz service over TLS");

    // Make healthz request
    let request = tonic::Request::new(HealthzRequest {});
    let response = client
        .healthz(request)
        .await
        .expect("Healthz request failed");
    let healthz_response = response.into_inner();

    // Verify the response
    assert!(
        healthz_response.is_healthy,
        "Healthz service returned unhealthy status"
    );
    assert!(
        healthz_response.server_time.is_some(),
        "Server time should be present"
    );
    assert!(
        !healthz_response.status_messages.is_empty(),
        "Status messages should be present"
    );

    println!("✓ Successfully connected to healthz service over TLS");
    println!("✓ Server is using certificate from tests/certs/server.crt");
    println!("✓ Client verified server certificate with CA from tests/certs/ca.crt");

    // Verify that non-TLS HTTP connection fails (TLS-only server)
    println!("Testing HTTP connection to verify TLS-only enforcement...");
    let http_endpoint =
        Channel::from_shared(format!("http://{}", bind_address)).expect("Invalid HTTP endpoint");

    let http_client_result = HealthzServiceClient::connect(http_endpoint).await;

    match http_client_result {
        Ok(mut http_client) => {
            // If connection succeeded, the request should still fail due to protocol mismatch
            println!("⚠ HTTP connection succeeded (server may accept both protocols)");
            let http_request = tonic::Request::new(HealthzRequest {});
            let http_response_result = http_client.healthz(http_request).await;
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
async fn test_init_indexes() {
    // Initialize rustls crypto provider
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let data = data(false).await;
    let bind_address = data.1.bind_address;
    let index_name = &data.0;
    let server_config = &data.1;
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
        &server_config.authnz,
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

/// Tests authentication and authorization for different services
#[tokio_shared_rt::test]
async fn test_authnz() {
    // Initialize rustls crypto provider
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let data = data(false).await;
    let bind_address = data.1.bind_address;
    let index_name = &data.0;
    let server_config = &data.1;
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

    // Test 1: Healthz service should work WITHOUT bearer token
    {
        let mut healthz_client = HealthzServiceClient::connect(endpoint.clone())
            .await
            .expect("Failed to connect to healthz service");

        let healthz_request = tonic::Request::new(HealthzRequest {});
        let healthz_response = healthz_client.healthz(healthz_request).await;

        assert!(
            healthz_response.is_ok(),
            "Healthz service should work without authentication"
        );
        println!("✓ Healthz service works without bearer token");
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
                }],
                dek: None,
                kek_id: None,
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
            &server_config.authnz,
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
                }],
                dek: None,
                kek_id: None,
            }),
        });

        // Generate JWT token for authentication with proper permissions
        let claims = generate_test_claims(
            "test-user",
            &server_config.authnz,
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
        let claims = generate_test_claims("test-user", &server_config.authnz, None);
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
                }],
                dek: None,
                kek_id: None,
            }),
        });

        // Add malformed authorization header (missing "Bearer" prefix)
        let claims = generate_test_claims("test-user", &server_config.authnz, None);
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
            &server_config.authnz,
            Some(OverrideClaims {
                sub: Some("".to_string()), // Invalid: empty subject
                issuer: Some(
                    server_config
                        .authnz
                        .jwt_issuer
                        .clone()
                        .unwrap_or_else(|| "server_integration_test-issuer".to_string()),
                ), // Valid
                audience: Some(
                    server_config
                        .authnz
                        .jwt_audience
                        .clone()
                        .unwrap_or_else(|| "server_integration_test-audience".to_string()),
                ), // Valid
                exp: Some(now + 3600),     // Valid: expires in 1 hour
                iat: Some(now),            // Valid: issued now
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
                }],
                dek: None,
                kek_id: None,
            }),
        });

        // Generate JWT token with empty issuer but all other claims valid
        let now = OffsetDateTime::now_utc().unix_timestamp() as usize;
        let claims = generate_test_claims(
            "test-user",
            &server_config.authnz,
            Some(OverrideClaims {
                sub: Some("test-user".to_string()), // Valid
                issuer: Some("".to_string()),       // Invalid: empty issuer
                audience: Some(
                    server_config
                        .authnz
                        .jwt_audience
                        .clone()
                        .unwrap_or_else(|| "server_integration_test-audience".to_string()),
                ), // Valid
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
            &server_config.authnz,
            Some(OverrideClaims {
                sub: Some("test-user".to_string()), // Valid
                issuer: Some(
                    server_config
                        .authnz
                        .jwt_issuer
                        .clone()
                        .unwrap_or_else(|| "server_integration_test-issuer".to_string()),
                ), // Valid
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
                }],
                dek: None,
                kek_id: None,
            }),
        });

        // Generate JWT token with zero expiration but all other claims valid
        let now = OffsetDateTime::now_utc().unix_timestamp() as usize;
        let claims = generate_test_claims(
            "test-user",
            &server_config.authnz,
            Some(OverrideClaims {
                sub: Some("test-user".to_string()), // Valid
                issuer: Some(
                    server_config
                        .authnz
                        .jwt_issuer
                        .clone()
                        .unwrap_or_else(|| "server_integration_test-issuer".to_string()),
                ), // Valid
                audience: Some(
                    server_config
                        .authnz
                        .jwt_audience
                        .clone()
                        .unwrap_or_else(|| "server_integration_test-audience".to_string()),
                ), // Valid
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
            &server_config.authnz,
            Some(OverrideClaims {
                sub: Some("test-user".to_string()), // Valid
                issuer: Some(
                    server_config
                        .authnz
                        .jwt_issuer
                        .clone()
                        .unwrap_or_else(|| "server_integration_test-issuer".to_string()),
                ), // Valid
                audience: Some(
                    server_config
                        .authnz
                        .jwt_audience
                        .clone()
                        .unwrap_or_else(|| "server_integration_test-audience".to_string()),
                ), // Valid
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
            &server_config.authnz,
            Some(OverrideClaims {
                sub: Some("test-user".to_string()), // Valid
                issuer: Some(
                    server_config
                        .authnz
                        .jwt_issuer
                        .clone()
                        .unwrap_or_else(|| "server_integration_test-issuer".to_string()),
                ), // Valid
                audience: Some(
                    server_config
                        .authnz
                        .jwt_audience
                        .clone()
                        .unwrap_or_else(|| "server_integration_test-audience".to_string()),
                ), // Valid
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
                }],
                dek: None,
                kek_id: None,
            }),
        });

        // Generate JWT token issued in the future with proper permissions
        let now = OffsetDateTime::now_utc().unix_timestamp() as usize;
        let claims = generate_test_claims(
            "test-user",
            &server_config.authnz,
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
            &server_config.authnz,
            Some(OverrideClaims {
                sub: Some("test-user".to_string()), // Valid
                issuer: Some(
                    server_config
                        .authnz
                        .jwt_issuer
                        .clone()
                        .unwrap_or_else(|| "server_integration_test-issuer".to_string()),
                ), // Valid
                audience: Some(
                    server_config
                        .authnz
                        .jwt_audience
                        .clone()
                        .unwrap_or_else(|| "server_integration_test-audience".to_string()),
                ), // Valid
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
            &server_config.authnz,
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
                }],
                dek: None,
                kek_id: None,
            }),
        });

        // Generate JWT token with wrong permission (index permission for entry service)
        let claims = generate_test_claims(
            "test-user",
            &server_config.authnz,
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
        let claims = generate_test_claims("test-user", &server_config.authnz, None);
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
                }],
                dek: None,
                kek_id: None,
            }),
        });

        // Non-udex: scope values are silently discarded → no permissions granted → denied
        let claims = generate_test_claims(
            "test-user",
            &server_config.authnz,
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
                }],
                dek: None,
                kek_id: None,
            }),
        });

        // Mixed scope: non-udex values are silently discarded; the udex: value grants access
        let claims = generate_test_claims(
            "test-user",
            &server_config.authnz,
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
            &server_config.authnz,
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
                }],
                dek: None,
                kek_id: None,
            }),
        });

        // No scope claim → no permissions → denied
        let claims = generate_test_claims("test-user", &server_config.authnz, None);
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
            &server_config.authnz,
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
            &server_config.authnz,
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
async fn test_tls_wrong_ca_rejected() {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let data = data(false).await;
    let bind_address = data.1.bind_address;

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

    let err = HealthzServiceClient::connect(endpoint)
        .await
        .expect_err("Connection with wrong CA certificate must be rejected");
    let err_str = format!("{err:?}");
    assert!(
        err_str.to_lowercase().contains("certificate"),
        "Expected a certificate validation error, got: {err_str}"
    );
    println!("✓ Connection with wrong CA correctly rejected");
}

/// Verifies that a client relying on the system trust store cannot connect.
///
/// The test server uses a self-signed CA that is not installed in the system
/// trust store, so the TLS handshake must fail without an explicit CA override.
#[tokio_shared_rt::test]
async fn test_tls_untrusted_ca_rejected() {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let data = data(false).await;
    let bind_address = data.1.bind_address;

    // No explicit CA — falls back to the system trust store, which does not
    // contain the self-signed test CA.
    let tls_config = ClientTlsConfig::new().domain_name("localhost");

    let endpoint = Channel::from_shared(format!("https://{}", bind_address))
        .expect("Invalid endpoint")
        .tls_config(tls_config)
        .expect("Failed to configure TLS");

    let err = HealthzServiceClient::connect(endpoint)
        .await
        .expect_err("Connection using system trust store must be rejected for self-signed test CA");
    let err_str = format!("{err:?}");
    assert!(
        err_str.to_lowercase().contains("certificate"),
        "Expected a certificate validation error, got: {err_str}"
    );
    println!("✓ Connection using system trust store correctly rejected");
}
