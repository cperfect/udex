/// Integration tests for the entry server
use maybe_once::tokio::{Data, MaybeOnceAsync};
use rstest::*;
use std::sync::{Arc, OnceLock};
use udex_api::entry::entry_service_server::EntryService as EntryServiceTrait;
use udex_api::index::HashAlgorithm;
use udex_datastore::integration_test::init_postgres;
use udex_datastore::postgres::PostgresDatastore;
use udex_server::{logging, EntryService, IndexService};
use uuid::Uuid;

const ID_PREFIX: &str = "entry_service_integration_test_";

// See https://github.com/ufoscout/maybe-once for the MaybeOnceAsync pattern used here.
type MaybeOnceType = (
    EntryService<PostgresDatastore>,
    String, // index name
    String, // database name for cleanup (kept in scope for automatic cleanup via ctor)
);

/// Initializer function.
/// Starts a Postgres container shared between all tests.
/// It will be stopped when the tests terminate.
async fn init_entry_service() -> MaybeOnceType {
    logging::init_test_tracing();

    let datastore_fixtures = init_postgres().await;
    let datastore = Arc::from(datastore_fixtures.0);

    // Use the existing datastore to create the index
    let index_name = format!("{}_index", ID_PREFIX);
    let (index_reporter, _) = tonic_health::server::health_reporter();
    let index_server: IndexService<PostgresDatastore> =
        IndexService::new(datastore.clone(), index_reporter);

    // statically define an index for testing
    let init_index = udex_api::index::UpdateIndexRequest {
        name: index_name.clone(),
        update: Some(udex_api::index::IndexUpdate {
            description: Some("Test entry description".to_string()),
            display_name: Some("Test Entry Index".to_string()),
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

    // Create a new datastore instance for the EntryService from the same pool
    let (entry_reporter, _) = tonic_health::server::health_reporter();
    let entry_service: EntryService<PostgresDatastore> =
        EntryService::new(datastore.clone(), entry_reporter);

    entry_service
        .init(Arc::new(index_server))
        .await
        .expect("Failed to initialize entry service");

    (entry_service, index_name, datastore_fixtures.2)
}

/// A function that holds a static reference to the container
pub async fn data(serial: bool) -> Data<'static, MaybeOnceType> {
    static DATA: OnceLock<MaybeOnceAsync<MaybeOnceType>> = OnceLock::new();
    DATA.get_or_init(|| MaybeOnceAsync::new(|| Box::pin(init_entry_service())))
        .data(serial)
        .await
}

/// Tests that the entry server can be initialized successfully.
#[rstest]
#[tokio_shared_rt::test] //We use tokio shared runtime to ensure the static variables are still valid between tests -https://docs.rs/tokio-shared-rt/latest/tokio_shared_rt/
async fn test_entry_service_init() {
    // Verifies that init_entry_service() completes without panicking.
    // Health status is verified end-to-end via test_server_health_check in server_integration_tests.
    let _data = data(false).await;
}

/// Tests creating an entry through the gRPC service
#[rstest]
#[tokio_shared_rt::test]
async fn test_entry_service_create_entry() {
    use tonic::Request;
    use udex_api::entry::{ContextInput, CreateEntryRequest, KeyValuePair, Value};

    let data = data(false).await;
    let entry_server = &data.0;
    let index_name = &data.1;

    // Create test context input
    let context_input = ContextInput {
        pairs: vec![
            KeyValuePair {
                key: "user_id".to_string(),
                value: Some(Value {
                    value: Some(udex_api::entry::value::Value::StringValue(
                        "user123".to_string(),
                    )),
                }),
                kek_id: None,
                dek: None,
            },
            KeyValuePair {
                key: "session_id".to_string(),
                value: Some(Value {
                    value: Some(udex_api::entry::value::Value::StringValue(
                        "sess456".to_string(),
                    )),
                }),
                kek_id: None,
                dek: None,
            },
        ],
    };

    let request = Request::new(CreateEntryRequest {
        index_name: index_name.clone(),
        context: Some(context_input),
    });

    let response = entry_server
        .create_entry(request)
        .await
        .expect("Create entry failed");
    let create_response = response.into_inner();

    // Verify response
    assert!(!create_response.key.is_empty(), "Key should not be empty");
    assert!(
        !create_response.context_hash.is_empty(),
        "Context hash should not be empty"
    );

    // Verify key is valid UUID
    let _uuid = Uuid::parse_str(&create_response.key).expect("Key should be valid UUID");
}

/// Tests deleting an entry through the gRPC service
#[rstest]
#[tokio_shared_rt::test]
async fn test_entry_service_lookup_context_by_key() {
    use tonic::Request;
    use udex_api::entry::{
        ContextInput, CreateEntryRequest, KeyValuePair, LookupContextByKeyRequest, Value,
    };

    let data = data(false).await;
    let entry_server = &data.0;
    let index_name = &data.1;

    // Create an entry first
    let context_input = ContextInput {
        pairs: vec![KeyValuePair {
            key: "lookup_test".to_string(),
            value: Some(Value {
                value: Some(udex_api::entry::value::Value::StringValue(
                    "lookup_value".to_string(),
                )),
            }),
            kek_id: None,
            dek: None,
        }],
    };

    let create_request = Request::new(CreateEntryRequest {
        index_name: index_name.clone(),
        context: Some(context_input.clone()),
    });

    let create_response = entry_server
        .create_entry(create_request)
        .await
        .expect("Create entry failed");
    let key = create_response.into_inner().key;

    // Now lookup the context
    let lookup_request = Request::new(LookupContextByKeyRequest {
        index_name: index_name.clone(),
        key,
    });
    let lookup_response = entry_server
        .lookup_context_by_key(lookup_request)
        .await
        .expect("Lookup failed");
    let context = lookup_response
        .into_inner()
        .context
        .expect("Context should be present");

    // Verify context matches what we created
    assert_eq!(context.pairs.len(), 1);
    assert_eq!(context.pairs[0].key, "lookup_test");
    if let Some(ref value) = context.pairs[0].value {
        if let Some(udex_api::entry::value::Value::StringValue(ref s)) = value.value {
            assert_eq!(s, "lookup_value");
        } else {
            panic!("Expected string value");
        }
    } else {
        panic!("Expected value to be present");
    }
}

/// An empty BulkWriteEntryOperationRequest is rejected with INVALID_ARGUMENT.
#[rstest]
#[tokio_shared_rt::test]
async fn test_entry_service_bulk_write_empty_invalid_argument() {
    use tonic::{Code, Request};
    use udex_api::entry::BulkWriteEntryOperationRequest;

    let data = data(false).await;
    let entry_server = &data.0;
    let index_name = &data.1;

    let err = entry_server
        .bulk_write_entry_operation(Request::new(BulkWriteEntryOperationRequest {
            index_name: index_name.clone(),
            operations: vec![],
        }))
        .await
        .expect_err("empty bulk write must be rejected");

    assert_eq!(
        err.code(),
        Code::InvalidArgument,
        "empty bulk write must return INVALID_ARGUMENT"
    );
}

/// An empty BulkReadEntryOperationRequest is rejected with INVALID_ARGUMENT.
#[rstest]
#[tokio_shared_rt::test]
async fn test_entry_service_bulk_read_empty_invalid_argument() {
    use tonic::{Code, Request};
    use udex_api::entry::BulkReadEntryOperationRequest;

    let data = data(false).await;
    let entry_server = &data.0;
    let index_name = &data.1;

    let err = entry_server
        .bulk_read_entry_operation(Request::new(BulkReadEntryOperationRequest {
            index_name: index_name.clone(),
            operations: vec![],
        }))
        .await
        .expect_err("empty bulk read must be rejected");

    assert_eq!(
        err.code(),
        Code::InvalidArgument,
        "empty bulk read must return INVALID_ARGUMENT"
    );
}
#[rstest]
#[tokio_shared_rt::test]
async fn test_entry_service_error_handling() {
    use tonic::Request;
    use udex_api::entry::{CreateEntryRequest, DeleteEntryRequest, LookupContextByKeyRequest};

    let data = data(false).await;
    let entry_server = &data.0;
    let index_name = &data.1;

    // Test creating entry without context
    let create_request = Request::new(CreateEntryRequest {
        index_name: index_name.clone(),
        context: None,
    });
    let create_result = entry_server.create_entry(create_request).await;
    assert!(create_result.is_err(), "Should fail without context");

    // Test lookup with invalid key format
    let lookup_request = Request::new(LookupContextByKeyRequest {
        index_name: index_name.clone(),
        key: "invalid-uuid".to_string(),
    });
    let lookup_result = entry_server.lookup_context_by_key(lookup_request).await;
    assert!(lookup_result.is_err(), "Should fail with invalid UUID");

    // Test delete with invalid key format
    let delete_request = Request::new(DeleteEntryRequest {
        index_name: index_name.clone(),
        key: "invalid-uuid".to_string(),
    });
    let delete_result = entry_server.delete_entry(delete_request).await;
    assert!(delete_result.is_err(), "Should fail with invalid UUID");

    // Test lookup with non-existent key
    let valid_uuid = Uuid::new_v4().to_string();
    let lookup_request = Request::new(LookupContextByKeyRequest {
        index_name: index_name.clone(),
        key: valid_uuid,
    });
    let lookup_result = entry_server.lookup_context_by_key(lookup_request).await;
    assert!(lookup_result.is_err(), "Should fail for non-existent key");
}

// ---------------------------------------------------------------------------
// lookup_key_by_context_or_create tests
// ---------------------------------------------------------------------------

/// Helper: build a single-pair ContextInput and its server-computed hash.
fn loc_context(pair_value: &str) -> (udex_api::entry::ContextInput, String) {
    use udex_api::entry::{ContextInput, KeyValuePair, Value};
    use udex_api::hash::xxh3_context_hash;

    let ctx = ContextInput {
        pairs: vec![KeyValuePair {
            key: "loc_user".to_string(),
            value: Some(Value {
                value: Some(udex_api::entry::value::Value::StringValue(
                    pair_value.to_string(),
                )),
            }),
            kek_id: None,
            dek: None,
        }],
    };
    let hash = xxh3_context_hash(&ctx).expect("hash must succeed for valid context");
    (ctx, hash)
}

/// lookup_key_by_context_or_create: first call for an unseen context creates
/// the entry and returns created=true with a valid UUID key.
#[rstest]
#[tokio_shared_rt::test]
async fn test_entry_service_lookup_or_create_validation_errors() {
    use tonic::{Code, Request};
    use udex_api::entry::LookupKeyByContextOrCreateRequest;

    let data = data(false).await;
    let entry_server = &data.0;
    let index_name = &data.1;

    let (ctx, hash) = loc_context("loc_validation_user");

    // Missing context
    let err = entry_server
        .lookup_key_by_context_or_create(Request::new(LookupKeyByContextOrCreateRequest {
            index_name: index_name.clone(),
            context: None,
            context_hash: hash.clone(),
        }))
        .await
        .expect_err("missing context must be rejected");
    assert_eq!(
        err.code(),
        Code::InvalidArgument,
        "missing context → INVALID_ARGUMENT"
    );

    // Missing context_hash
    let err = entry_server
        .lookup_key_by_context_or_create(Request::new(LookupKeyByContextOrCreateRequest {
            index_name: index_name.clone(),
            context: Some(ctx.clone()),
            context_hash: String::new(),
        }))
        .await
        .expect_err("empty context_hash must be rejected");
    assert_eq!(
        err.code(),
        Code::InvalidArgument,
        "empty context_hash → INVALID_ARGUMENT"
    );

    // Empty index_name
    let err = entry_server
        .lookup_key_by_context_or_create(Request::new(LookupKeyByContextOrCreateRequest {
            index_name: String::new(),
            context: Some(ctx),
            context_hash: hash,
        }))
        .await
        .expect_err("empty index_name must be rejected");
    assert_eq!(
        err.code(),
        Code::InvalidArgument,
        "empty index_name → INVALID_ARGUMENT"
    );
}

/// Regression: an index created at runtime (not via init_indexes) must be
/// usable for create_entry without a server restart.
///
/// Previously, EntryService.index_hasher_fns was only populated during init(),
/// so any index created after startup caused a "hash function not found" error.
#[rstest]
#[tokio_shared_rt::test]
async fn test_entry_service_create_entry_after_runtime_create_index() {
    use tonic::Request;
    use udex_api::entry::{ContextInput, CreateEntryRequest, KeyValuePair, Value};
    use udex_api::index::{HashAlgorithm, Index};
    use udex_datastore::Datastore;

    logging::init_test_tracing();

    let datastore_fixtures = init_postgres().await;
    let datastore = Arc::new(datastore_fixtures.0);

    // Initialise both services with no pre-configured indices — hasher cache starts empty.
    let (idx_reporter, _) = tonic_health::server::health_reporter();
    let index_service = IndexService::new(datastore.clone(), idx_reporter);
    index_service
        .init(vec![])
        .await
        .expect("index service init must succeed");

    let (entry_reporter, _) = tonic_health::server::health_reporter();
    let entry_service: EntryService<PostgresDatastore> =
        EntryService::new(datastore.clone(), entry_reporter);
    entry_service
        .init(Arc::new(index_service))
        .await
        .expect("entry service init must succeed");

    // Simulate a runtime create_index by writing directly to the datastore —
    // the same path the IndexService gRPC handler takes.
    let runtime_index_name = format!("{}_runtime_index", ID_PREFIX);
    datastore
        .create_index(Index {
            name: runtime_index_name.clone(),
            description: "runtime-created index".to_string(),
            display_name: "Runtime Index".to_string(),
            max_bulk_operations: 10,
            max_key_length: 256,
            max_value_length: 1024,
            max_kv_pairs_per_context: 5,
            hash_algorithm: HashAlgorithm::Xxh3 as i32,
            created_at: Some(udex_api::now_timestamp()),
            created_by: "test".to_string(),
            updated_at: None,
            updated_by: None,
        })
        .await
        .expect("runtime create_index must succeed");

    // create_entry must succeed without a restart — the lazy DB lookup populates the cache.
    let response = entry_service
        .create_entry(Request::new(CreateEntryRequest {
            index_name: runtime_index_name,
            context: Some(ContextInput {
                pairs: vec![KeyValuePair {
                    key: "user_id".to_string(),
                    value: Some(Value {
                        value: Some(udex_api::entry::value::Value::StringValue(
                            "runtime_user".to_string(),
                        )),
                    }),
                    kek_id: None,
                    dek: None,
                }],
            }),
        }))
        .await
        .expect("create_entry on a runtime-created index must succeed");

    let inner = response.into_inner();
    assert!(!inner.key.is_empty(), "key must not be empty");
    assert!(
        !inner.context_hash.is_empty(),
        "context_hash must not be empty"
    );
    Uuid::parse_str(&inner.key).expect("key must be a valid UUID");
}

/// lookup_key_by_context_or_create: a context_hash that does not match the
/// server-computed hash returns INVALID_ARGUMENT immediately, before any
/// database access — even when the entry does not exist.
#[rstest]
#[tokio_shared_rt::test]
async fn test_entry_service_lookup_or_create_hash_mismatch() {
    use tonic::{Code, Request};
    use udex_api::entry::LookupKeyByContextOrCreateRequest;

    let data = data(false).await;
    let entry_server = &data.0;
    let index_name = &data.1;

    let (ctx, _correct_hash) = loc_context("loc_mismatch_user");

    let err = entry_server
        .lookup_key_by_context_or_create(Request::new(LookupKeyByContextOrCreateRequest {
            index_name: index_name.clone(),
            context: Some(ctx),
            context_hash: "deliberately-wrong-hash".to_string(),
        }))
        .await
        .expect_err("hash mismatch must be rejected");

    assert_eq!(
        err.code(),
        Code::InvalidArgument,
        "hash mismatch must return INVALID_ARGUMENT"
    );
    assert!(
        err.message().contains("context_hash mismatch"),
        "error message must identify the mismatch: {}",
        err.message()
    );
}
