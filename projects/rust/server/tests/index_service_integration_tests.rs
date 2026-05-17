/// Integration tests for the index service
use maybe_once::tokio::{Data, MaybeOnceAsync};
use rstest::*;
use std::sync::{Arc, OnceLock};
use tonic::Request;
use udex_api::authz::claims::Claims;
use udex_api::index::{
    index_service_server::IndexService as IndexServiceTrait, CreateIndexRequest,
    DeleteIndexRequest, DescribeRequest, HashAlgorithm, IndexUpdate, UpdateIndexRequest,
};
use udex_datastore::integration_test::init_postgres;
use udex_datastore::postgres::PostgresDatastore;
use udex_server::{logging, IndexService};

// See https://github.com/ufoscout/maybe-once for the MaybeOnceAsync pattern used here.
type MaybeOnceType = (
    IndexService<PostgresDatastore>,
    String, // database name for cleanup (kept in scope for automatic cleanup via ctor)
);

/// Initializer function.
/// Starts a Postgres container shared between all tests.
/// It will be stopped when the tests terminate.
async fn init_index_service() -> MaybeOnceType {
    logging::init_test_tracing();

    let datastore_fixtures = init_postgres().await;
    let datastore = Arc::from(datastore_fixtures.0);

    let index_server: IndexService<PostgresDatastore> = IndexService::new(datastore);

    // statically define an index for testing
    let init_index = udex_api::index::UpdateIndexRequest {
        name: "test_index".to_string(),
        update: Some(udex_api::index::IndexUpdate {
            description: Some("Test index description".to_string()),
            display_name: Some("Test Index".to_string()),
            max_bulk_operations: Some(100),
            max_key_length: Some(256),
            max_value_length: Some(1024),
            max_kv_pairs_per_context: Some(10),
            hash_algorithm: Some(HashAlgorithm::Xxh3 as i32),
        }),
    };

    // initialize the index service with the static index
    index_server
        .init(vec![init_index.clone()])
        .await
        .expect("Failed to initialize index service");

    (index_server, datastore_fixtures.2)
}

/// A function that holds a static reference to the container
pub async fn data(serial: bool) -> Data<'static, MaybeOnceType> {
    static DATA: OnceLock<MaybeOnceAsync<MaybeOnceType>> = OnceLock::new();
    DATA.get_or_init(|| MaybeOnceAsync::new(|| Box::pin(init_index_service())))
        .data(serial)
        .await
}

/// Inserts a synthetic Claims into a request's extensions, simulating what the
/// auth interceptor would do. Required because create_index now rejects requests
/// without Claims rather than defaulting to an "unknown" subject.
fn with_test_claims<T>(mut request: Request<T>) -> Request<T> {
    let iat = time::OffsetDateTime::now_utc().unix_timestamp() as usize;
    let exp = iat + 3600; // 1 hour from now
    let claims = Claims::new(
        "test-subject".to_string(),
        "test-issuer".to_string(),
        "test-audience".to_string(),
        exp,
        iat,
    );
    request.extensions_mut().insert(claims);
    request
}

/// Tests that the index server can be initialized successfully.
#[rstest]
#[tokio_shared_rt::test]
async fn test_index_service_describe_empty_name() {
    let data = data(false).await;
    let index_server = &data.0;

    let request = Request::new(DescribeRequest {
        name: "".to_string(),
    });
    let result = index_server.describe(request).await;

    assert!(
        result.is_err(),
        "Describe with empty name should return an error"
    );

    if let Err(status) = result {
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
        assert!(status.message().contains("index name is required"));
    }
}

/// Tests the create_index endpoint with valid input creates the index and returns it
#[rstest]
#[tokio_shared_rt::test]
async fn test_index_service_create_unsupported_hash_algorithm() {
    let data = data(false).await;
    let index_server = &data.0;

    let request = Request::new(CreateIndexRequest {
        name: "unsupported_hash_index".to_string(),
        display_name: "Unsupported Hash Index".to_string(),
        description: "Should fail".to_string(),
        max_bulk_operations: 10,
        max_key_length: 64,
        max_value_length: 256,
        max_kv_pairs_per_context: 5,
        hash_algorithm: 99, // not a valid HashAlgorithm variant
    });
    let result = index_server.create_index(with_test_claims(request)).await;

    assert!(
        result.is_err(),
        "Create index with unknown hash_algorithm value should fail"
    );
    let status = result.unwrap_err();
    assert_eq!(status.code(), tonic::Code::InvalidArgument);
    assert!(status.message().contains("unsupported hash_algorithm"));
}

/// Tests the create_index endpoint rejects a duplicate index name
#[rstest]
#[tokio_shared_rt::test]
async fn test_index_service_create_empty_name() {
    let data = data(false).await;
    let index_server = &data.0;

    let request = Request::new(CreateIndexRequest {
        name: "".to_string(),
        display_name: "Test Index".to_string(),
        description: "Test index description".to_string(),
        max_bulk_operations: 100,
        max_key_length: 256,
        max_value_length: 1024,
        max_kv_pairs_per_context: 50,
        hash_algorithm: HashAlgorithm::Xxh3 as i32,
    });
    let result = index_server.create_index(with_test_claims(request)).await;

    assert!(
        result.is_err(),
        "Create index with empty name should return an error"
    );

    if let Err(status) = result {
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
        assert!(status.message().contains("index name is required"));
    }
}

/// Tests the create_index endpoint validation for invalid max_bulk_operations
#[rstest]
#[tokio_shared_rt::test]
async fn test_index_service_create_invalid_max_bulk_operations() {
    let data = data(false).await;
    let index_server = &data.0;

    let request = Request::new(CreateIndexRequest {
        name: "test_index".to_string(),
        display_name: "Test Index".to_string(),
        description: "Test index description".to_string(),
        max_bulk_operations: 0, // Invalid: should be >= 1
        max_key_length: 256,
        max_value_length: 1024,
        max_kv_pairs_per_context: 50,
        hash_algorithm: HashAlgorithm::Xxh3 as i32,
    });
    let result = index_server.create_index(with_test_claims(request)).await;

    assert!(
        result.is_err(),
        "Create index with invalid max_bulk_operations should return an error"
    );

    if let Err(status) = result {
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
        assert!(status
            .message()
            .contains("max_bulk_operations must be >= 1"));
    }
}

/// Tests the create_index endpoint validation for invalid max_key_length
#[rstest]
#[tokio_shared_rt::test]
async fn test_index_service_create_invalid_max_key_length() {
    let data = data(false).await;
    let index_server = &data.0;

    let request = Request::new(CreateIndexRequest {
        name: "test_index".to_string(),
        display_name: "Test Index".to_string(),
        description: "Test index description".to_string(),
        max_bulk_operations: 100,
        max_key_length: 0, // Invalid: should be >= 1
        max_value_length: 1024,
        max_kv_pairs_per_context: 50,
        hash_algorithm: HashAlgorithm::Xxh3 as i32,
    });
    let result = index_server.create_index(with_test_claims(request)).await;

    assert!(
        result.is_err(),
        "Create index with invalid max_key_length should return an error"
    );

    if let Err(status) = result {
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
        assert!(status.message().contains("max_key_length must be >= 1"));
    }
}

/// Tests the create_index endpoint validation for invalid max_value_length
#[rstest]
#[tokio_shared_rt::test]
async fn test_index_service_create_invalid_max_value_length() {
    let data = data(false).await;
    let index_server = &data.0;

    let request = Request::new(CreateIndexRequest {
        name: "test_index".to_string(),
        display_name: "Test Index".to_string(),
        description: "Test index description".to_string(),
        max_bulk_operations: 100,
        max_key_length: 256,
        max_value_length: 0, // Invalid: should be >= 1
        max_kv_pairs_per_context: 50,
        hash_algorithm: HashAlgorithm::Xxh3 as i32,
    });
    let result = index_server.create_index(with_test_claims(request)).await;

    assert!(
        result.is_err(),
        "Create index with invalid max_value_length should return an error"
    );

    if let Err(status) = result {
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
        assert!(status.message().contains("max_value_length must be >= 1"));
    }
}

/// Tests the create_index endpoint validation for invalid max_kv_pairs_per_context
#[rstest]
#[tokio_shared_rt::test]
async fn test_index_service_create_invalid_max_kv_pairs_per_context() {
    let data = data(false).await;
    let index_server = &data.0;

    let request = Request::new(CreateIndexRequest {
        name: "test_index".to_string(),
        display_name: "Test Index".to_string(),
        description: "Test index description".to_string(),
        max_bulk_operations: 100,
        max_key_length: 256,
        max_value_length: 1024,
        max_kv_pairs_per_context: 0, // Invalid: should be >= 1
        hash_algorithm: HashAlgorithm::Xxh3 as i32,
    });
    let result = index_server.create_index(with_test_claims(request)).await;

    assert!(
        result.is_err(),
        "Create index with invalid max_kv_pairs_per_context should return an error"
    );

    if let Err(status) = result {
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
        assert!(status
            .message()
            .contains("max_kv_pairs_per_context must be >= 1"));
    }
}

/// Tests the create_index endpoint rejects an unknown hash_algorithm enum value
#[rstest]
#[tokio_shared_rt::test]
async fn test_index_service_create_invalid_hash_algorithm() {
    let data = data(false).await;
    let index_server = &data.0;

    let request = Request::new(CreateIndexRequest {
        name: "test_index".to_string(),
        display_name: "Test Index".to_string(),
        description: "Test index description".to_string(),
        max_bulk_operations: 100,
        max_key_length: 256,
        max_value_length: 1024,
        max_kv_pairs_per_context: 50,
        hash_algorithm: 99, // not a valid HashAlgorithm variant
    });
    let result = index_server.create_index(with_test_claims(request)).await;

    assert!(
        result.is_err(),
        "Create index with invalid hash_algorithm should return an error"
    );

    if let Err(status) = result {
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
        assert!(status.message().contains("unsupported hash_algorithm"));
    }
}

/// Tests that index names with invalid characters are rejected
#[rstest]
#[tokio_shared_rt::test]
async fn test_index_service_create_invalid_name_chars() {
    let data = data(false).await;
    let index_server = &data.0;

    let invalid_names = vec![
        "has space",
        "has@symbol",
        "has.dot",
        "has/slash",
        "has!bang",
    ];

    for name in invalid_names {
        let request = Request::new(CreateIndexRequest {
            name: name.to_string(),
            display_name: "Test Index".to_string(),
            description: "Test description".to_string(),
            max_bulk_operations: 100,
            max_key_length: 256,
            max_value_length: 1024,
            max_kv_pairs_per_context: 50,
            hash_algorithm: HashAlgorithm::Xxh3 as i32,
        });
        let result = index_server.create_index(with_test_claims(request)).await;
        assert!(result.is_err(), "Name '{name}' should be rejected");
        assert_eq!(
            result.unwrap_err().code(),
            tonic::Code::InvalidArgument,
            "Name '{name}' should return InvalidArgument"
        );
    }
}

/// Tests that valid name characters (Unicode letters, digits, hyphens, underscores) are accepted
#[rstest]
#[tokio_shared_rt::test]
async fn test_index_service_create_valid_name_chars() {
    let data = data(false).await;
    let index_server = &data.0;

    let valid_names = vec![
        "simple",
        "with-hyphen",
        "with_underscore",
        "MixedCase",
        "123digits",
        "a-b_c1",
    ];

    for name in valid_names {
        let request = Request::new(CreateIndexRequest {
            name: name.to_string(),
            display_name: "Test Index".to_string(),
            description: "Test description".to_string(),
            max_bulk_operations: 100,
            max_key_length: 256,
            max_value_length: 1024,
            max_kv_pairs_per_context: 50,
            hash_algorithm: HashAlgorithm::Xxh3 as i32,
        });
        let result = index_server.create_index(with_test_claims(request)).await;
        assert!(
            result.is_ok(),
            "Name '{name}' should be accepted: {:?}",
            result.err()
        );
    }
}

/// Tests that empty display_name is rejected
#[rstest]
#[tokio_shared_rt::test]
async fn test_index_service_create_empty_display_name() {
    let data = data(false).await;
    let index_server = &data.0;

    for display_name in ["", "   "] {
        let request = Request::new(CreateIndexRequest {
            name: "valid-name".to_string(),
            display_name: display_name.to_string(),
            description: "Test description".to_string(),
            max_bulk_operations: 100,
            max_key_length: 256,
            max_value_length: 1024,
            max_kv_pairs_per_context: 50,
            hash_algorithm: HashAlgorithm::Xxh3 as i32,
        });
        let result = index_server.create_index(with_test_claims(request)).await;
        assert!(
            result.is_err(),
            "Empty display_name '{display_name}' should be rejected"
        );
        let status = result.unwrap_err();
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
        assert!(status.message().contains("display_name is required"));
    }
}

/// Tests that empty description is rejected
#[rstest]
#[tokio_shared_rt::test]
async fn test_index_service_create_empty_description() {
    let data = data(false).await;
    let index_server = &data.0;

    for description in ["", "   "] {
        let request = Request::new(CreateIndexRequest {
            name: "valid-name".to_string(),
            display_name: "Valid Display Name".to_string(),
            description: description.to_string(),
            max_bulk_operations: 100,
            max_key_length: 256,
            max_value_length: 1024,
            max_kv_pairs_per_context: 50,
            hash_algorithm: HashAlgorithm::Xxh3 as i32,
        });
        let result = index_server.create_index(with_test_claims(request)).await;
        assert!(
            result.is_err(),
            "Empty description '{description}' should be rejected"
        );
        let status = result.unwrap_err();
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
        assert!(status.message().contains("description is required"));
    }
}

/// Tests the update_index endpoint with valid input returns NotImplemented
#[rstest]
#[tokio_shared_rt::test]
async fn test_index_service_update_empty_name() {
    let data = data(false).await;
    let index_server = &data.0;

    let request = Request::new(UpdateIndexRequest {
        name: "".to_string(),
        update: Some(IndexUpdate {
            description: Some("Updated test index description".to_string()),
            display_name: None,
            max_bulk_operations: None,
            max_key_length: None,
            max_value_length: None,
            max_kv_pairs_per_context: None,
            hash_algorithm: None,
        }),
    });
    let result = index_server.update_index(request).await;

    assert!(
        result.is_err(),
        "Update index with empty name should return an error"
    );

    if let Err(status) = result {
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
        assert!(status.message().contains("index name is required"));
    }
}

/// Tests the list_indices endpoint returns success with initialized index
#[rstest]
#[tokio_shared_rt::test]
async fn test_index_service_update_missing_update() {
    let data = data(false).await;
    let index_server = &data.0;

    let request = Request::new(UpdateIndexRequest {
        name: "test_index".to_string(),
        update: None,
    });
    let result = index_server.update_index(request).await;

    assert!(
        result.is_err(),
        "Update index with missing update should return an error"
    );

    if let Err(status) = result {
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
        assert!(status.message().contains("update fields are required"));
    }
}

/// Tests update index with empty update fields
#[rstest]
#[tokio_shared_rt::test]
async fn test_index_service_update_empty_update() {
    let data = data(false).await;
    let index_server = &data.0;

    let request = Request::new(UpdateIndexRequest {
        name: "test_index".to_string(),
        update: Some(IndexUpdate {
            description: None,
            display_name: None,
            max_bulk_operations: None,
            max_key_length: None,
            max_value_length: None,
            max_kv_pairs_per_context: None,
            hash_algorithm: None,
        }),
    });
    let result = index_server.update_index(request).await;

    assert!(
        result.is_err(),
        "Update index with empty update should return an error"
    );

    if let Err(status) = result {
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
        assert!(status
            .message()
            .contains("at least one field must be provided"));
    }
}

/// Tests multiple validation errors in sequence to verify error handling consistency
#[rstest]
#[tokio_shared_rt::test]
async fn test_index_service_validation_error_consistency() {
    let data = data(false).await;
    let index_server = &data.0;

    // Test multiple invalid create requests to ensure consistent error handling
    let invalid_requests = vec![
        CreateIndexRequest {
            name: "".to_string(),
            display_name: "Test".to_string(),
            description: "Test".to_string(),
            max_bulk_operations: 100,
            max_key_length: 256,
            max_value_length: 1024,
            max_kv_pairs_per_context: 50,
            hash_algorithm: HashAlgorithm::Xxh3 as i32,
        },
        CreateIndexRequest {
            name: "valid_name".to_string(),
            display_name: "Test".to_string(),
            description: "Test".to_string(),
            max_bulk_operations: -1,
            max_key_length: 256,
            max_value_length: 1024,
            max_kv_pairs_per_context: 50,
            hash_algorithm: HashAlgorithm::Xxh3 as i32,
        },
        CreateIndexRequest {
            name: "valid_name".to_string(),
            display_name: "Test".to_string(),
            description: "Test".to_string(),
            max_bulk_operations: 100,
            max_key_length: 256,
            max_value_length: 1024,
            max_kv_pairs_per_context: 50,
            hash_algorithm: 99, // not a valid HashAlgorithm variant
        },
    ];

    for invalid_request in invalid_requests {
        let request = Request::new(invalid_request);
        let result = index_server.create_index(with_test_claims(request)).await;

        assert!(result.is_err(), "Invalid request should return an error");

        if let Err(status) = result {
            assert_eq!(status.code(), tonic::Code::InvalidArgument);
            // All validation errors should be InvalidArgument
        }
    }
}

#[rstest]
#[tokio_shared_rt::test]
async fn test_index_service_delete_empty_name() {
    let data = data(false).await;
    let index_server = &data.0;

    let result = index_server
        .delete_index(with_test_claims(Request::new(DeleteIndexRequest {
            name: "".to_string(),
        })))
        .await;

    let status = result.unwrap_err();
    assert_eq!(
        status.code(),
        tonic::Code::InvalidArgument,
        "Empty index name should return InvalidArgument"
    );
}

#[rstest]
#[tokio_shared_rt::test]
async fn test_index_service_delete_required_permissions_and_missing_claims() {
    let data = data(false).await;
    let index_server = &data.0;

    // Use AuthorizorWrapper path: call via the authorizor with wrong permission
    // The direct IndexService skips authz; test the Permissable impl separately.
    // Verify the permission string is correct by checking Permissable directly.
    use udex_api::authz::permissions::Permissable;
    let req = DeleteIndexRequest {
        name: "my-index".to_string(),
    };
    let perms = req.required_permissions();
    assert_eq!(
        perms,
        vec!["udex:index:v1:my-index:delete"],
        "DeleteIndexRequest must require the delete permission scoped to the index name"
    );

    // Confirm a request without Claims is rejected.
    let result = index_server
        .delete_index(Request::new(DeleteIndexRequest {
            name: "test_index".to_string(),
        }))
        .await;
    let status = result.unwrap_err();
    assert_eq!(
        status.code(),
        tonic::Code::Internal,
        "Missing Claims should return Internal (auth middleware not applied)"
    );
}
