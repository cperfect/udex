/// Integration tests for the index service
use maybe_once::tokio::{Data, MaybeOnceAsync};
use rstest::*;
use std::sync::{Arc, OnceLock};
use tonic::Request;
use udex_api::authz::claims::Claims;
use udex_api::index::{
    index_service_server::IndexService as IndexServiceTrait, CreateIndexRequest,
    DeleteIndexRequest, DescribeRequest, HashAlgorithm, IndexUpdate, ListIndicesRequest,
    UpdateIndexRequest,
};
use udex_datastore::integration_test::init_postgres;
use udex_datastore::postgres::PostgresDatastore;
use udex_datastore::{Datastore, Entry};
use udex_server::{logging, HealthCheck, IndexService};

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
#[tokio_shared_rt::test] //We use tokio shared runtime to ensure the static variables are still valid between tests -https://docs.rs/tokio-shared-rt/latest/tokio_shared_rt/
async fn test_index_server_init() {
    let data = data(false).await;
    let index_server = &data.0;

    // Check that the index server is healthy
    let is_healthy = index_server
        .is_healthy()
        .await
        .expect("Health check failed");
    assert!(is_healthy, "Index server should be healthy");
}

/// Tests the describe endpoint with valid input returns the existing index
#[rstest]
#[tokio_shared_rt::test]
async fn test_describe_valid_input() {
    let data = data(false).await;
    let index_server = &data.0;

    let request = Request::new(DescribeRequest {
        name: "test_index".to_string(),
    });
    let result = index_server.describe(request).await;

    // Should return success since the test_index was created during initialization
    assert!(
        result.is_ok(),
        "Describe should return success for existing index"
    );

    let response = result.unwrap().into_inner();
    assert!(
        response.index.is_some(),
        "Response should contain index data"
    );

    let index = response.index.unwrap();
    assert_eq!(index.name, "test_index");
}

/// Tests the describe endpoint with empty name returns InvalidArgument
#[rstest]
#[tokio_shared_rt::test]
async fn test_describe_empty_name() {
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
async fn test_create_index_valid_input() {
    let data = data(false).await;
    let index_server = &data.0;

    let request = Request::new(CreateIndexRequest {
        name: "created_test_index".to_string(),
        display_name: "Created Test Index".to_string(),
        description: "A newly created index".to_string(),
        max_bulk_operations: 50,
        max_key_length: 128,
        max_value_length: 512,
        max_kv_pairs_per_context: 20,
        hash_algorithm: HashAlgorithm::Xxh3 as i32,
    });
    let result = index_server.create_index(with_test_claims(request)).await;

    assert!(
        result.is_ok(),
        "Create index with valid input should succeed: {:?}",
        result.err()
    );

    let index = result
        .unwrap()
        .into_inner()
        .index
        .expect("Response should contain the created index");
    assert_eq!(index.name, "created_test_index");
    assert_eq!(index.description, "A newly created index");
    assert_eq!(index.max_bulk_operations, 50);
    assert_eq!(index.max_key_length, 128);
    assert_eq!(index.max_value_length, 512);
    assert_eq!(index.max_kv_pairs_per_context, 20);
    assert_eq!(index.hash_algorithm, HashAlgorithm::Xxh3 as i32);
    assert!(index.created_at.is_some());
}

/// Tests the create_index endpoint rejects an unknown hash algorithm enum value
#[rstest]
#[tokio_shared_rt::test]
async fn test_create_index_unsupported_hash_algorithm() {
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
async fn test_create_index_duplicate_name() {
    let data = data(true).await; // serial: true to avoid races with test_create_index_valid_input
    let index_server = &data.0;

    // First creation should succeed
    let request = Request::new(CreateIndexRequest {
        name: "duplicate_test_index".to_string(),
        display_name: "Duplicate Test Index".to_string(),
        description: "First".to_string(),
        max_bulk_operations: 10,
        max_key_length: 64,
        max_value_length: 256,
        max_kv_pairs_per_context: 5,
        hash_algorithm: HashAlgorithm::Xxh3 as i32,
    });
    let result = index_server.create_index(with_test_claims(request)).await;
    assert!(
        result.is_ok(),
        "First create should succeed: {:?}",
        result.err()
    );

    // Second creation with same name should fail
    let request = Request::new(CreateIndexRequest {
        name: "duplicate_test_index".to_string(),
        display_name: "Duplicate Test Index".to_string(),
        description: "Second".to_string(),
        max_bulk_operations: 10,
        max_key_length: 64,
        max_value_length: 256,
        max_kv_pairs_per_context: 5,
        hash_algorithm: HashAlgorithm::Xxh3 as i32,
    });
    let result = index_server.create_index(with_test_claims(request)).await;
    assert!(result.is_err(), "Duplicate create should fail");
    // Deliberately returns Internal rather than AlreadyExists: exposing AlreadyExists would
    // leak the existence of index names the caller may not have permission to know about.
    assert_eq!(result.unwrap_err().code(), tonic::Code::Internal);
}

/// Tests the create_index endpoint validation for empty name
#[rstest]
#[tokio_shared_rt::test]
async fn test_create_index_empty_name() {
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
async fn test_create_index_invalid_max_bulk_operations() {
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
async fn test_create_index_invalid_max_key_length() {
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
async fn test_create_index_invalid_max_value_length() {
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
async fn test_create_index_invalid_max_kv_pairs_per_context() {
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
async fn test_create_index_invalid_hash_algorithm() {
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

/// Tests the update_index endpoint with valid input returns NotImplemented
#[rstest]
#[tokio_shared_rt::test]
async fn test_update_index_valid_input() {
    let data = data(false).await;
    let index_server = &data.0;

    let request = Request::new(UpdateIndexRequest {
        name: "test_index_not_found".to_string(),
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
        "Update index should return an error (index doesn't exist)"
    );

    if let Err(status) = result {
        // Since we're yet to actually implement the functionality,
        // the error should Unimplemented as we are not checking the datastore.
        assert!(
            status.code() == tonic::Code::Unimplemented || status.code() == tonic::Code::NotFound,
            "Expected Unimplemented or NotFound error, got: {:?}",
            status.code()
        );
        println!("Error message: {}", status.message());
    }
}

/// Tests the update_index endpoint validation for empty name
#[rstest]
#[tokio_shared_rt::test]
async fn test_update_index_empty_name() {
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
async fn test_list_indices() {
    let data = data(false).await;
    let index_server = &data.0;

    let request = Request::new(ListIndicesRequest {});
    let result = index_server.list_indices(request).await;

    assert!(result.is_ok(), "List indices should return success");

    let response = result.unwrap().into_inner();
    assert!(
        !response.indices.is_empty(),
        "Should return at least one index (test_index)"
    );

    // Verify that the test index is in the list
    let test_index = response.indices.iter().find(|i| i.name == "test_index");
    assert!(
        test_index.is_some(),
        "test_index should be in the list of indices"
    );

    let test_index = test_index.unwrap();
    assert_eq!(test_index.description, "Test index description");
    assert_eq!(test_index.hash_algorithm, HashAlgorithm::Xxh3 as i32);
}

/// Tests update index with missing update fields
#[rstest]
#[tokio_shared_rt::test]
async fn test_update_index_missing_update() {
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
async fn test_update_index_empty_update() {
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
async fn test_validation_error_consistency() {
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
async fn test_delete_index_empty_index() {
    let fixtures = init_postgres().await;
    let datastore = Arc::new(fixtures.0);
    let _db_name = fixtures.2;
    let index_server = IndexService::new(datastore.clone());

    index_server
        .create_index(with_test_claims(Request::new(CreateIndexRequest {
            name: "del_test_empty".to_string(),
            display_name: "Delete Test Empty".to_string(),
            description: "delete test".to_string(),
            max_bulk_operations: 100,
            max_key_length: 256,
            max_value_length: 1024,
            max_kv_pairs_per_context: 10,
            hash_algorithm: HashAlgorithm::Xxh3 as i32,
        })))
        .await
        .expect("Failed to create index");

    let result = index_server
        .delete_index(with_test_claims(Request::new(DeleteIndexRequest {
            name: "del_test_empty".to_string(),
        })))
        .await;

    assert!(result.is_ok(), "Deleting an empty index should succeed");

    let not_found = index_server
        .describe(Request::new(DescribeRequest {
            name: "del_test_empty".to_string(),
        }))
        .await;
    assert_eq!(
        not_found.unwrap_err().code(),
        tonic::Code::NotFound,
        "Deleted index should not be found"
    );
}

#[rstest]
#[tokio_shared_rt::test]
async fn test_delete_index_not_empty() {
    let fixtures = init_postgres().await;
    let datastore = Arc::new(fixtures.0);
    let _db_name = fixtures.2;
    let index_server = IndexService::new(datastore.clone());

    index_server
        .create_index(with_test_claims(Request::new(CreateIndexRequest {
            name: "del_test_nonempty".to_string(),
            display_name: "Delete Test Non-empty".to_string(),
            description: "delete test".to_string(),
            max_bulk_operations: 100,
            max_key_length: 256,
            max_value_length: 1024,
            max_kv_pairs_per_context: 10,
            hash_algorithm: HashAlgorithm::Xxh3 as i32,
        })))
        .await
        .expect("Failed to create index");

    datastore
        .create_entry(Entry {
            key: uuid::Uuid::new_v4(),
            context: udex_api::entry::Context {
                hash: "del_nonempty_hash".to_string(),
                pairs: vec![],
            },
            index_name: "del_test_nonempty".to_string(),
        })
        .await
        .expect("Failed to create entry");

    let result = index_server
        .delete_index(with_test_claims(Request::new(DeleteIndexRequest {
            name: "del_test_nonempty".to_string(),
        })))
        .await;

    let status = result.unwrap_err();
    assert_eq!(
        status.code(),
        tonic::Code::FailedPrecondition,
        "Deleting a non-empty index should return FailedPrecondition"
    );
}

#[rstest]
#[tokio_shared_rt::test]
async fn test_delete_index_not_found() {
    let data = data(false).await;
    let index_server = &data.0;

    let result = index_server
        .delete_index(with_test_claims(Request::new(DeleteIndexRequest {
            name: "nonexistent_index_xyz".to_string(),
        })))
        .await;

    let status = result.unwrap_err();
    assert_eq!(
        status.code(),
        tonic::Code::NotFound,
        "Deleting a non-existent index should return NotFound"
    );
}

#[rstest]
#[tokio_shared_rt::test]
async fn test_delete_index_empty_name() {
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
async fn test_delete_index_required_permissions_and_missing_claims() {
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
