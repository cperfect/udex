/// Integration tests for the entry server
use maybe_once::tokio::{Data, MaybeOnceAsync};
use rstest::*;
use std::sync::{Arc, OnceLock};
use udex_api::entry::entry_service_server::EntryService as EntryServiceTrait;
use udex_api::index::HashAlgorithm;
use udex_datastore::integration_test::init_postgres;
use udex_datastore::postgres::PostgresDatastore;
use udex_server::{logging, EntryService, HealthCheck, IndexService};
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
    let index_server: IndexService<PostgresDatastore> = IndexService::new(datastore.clone());

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
    let mut entry_service: EntryService<PostgresDatastore> = EntryService::new(datastore.clone());

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
async fn test_entry_server_init() {
    let data = data(false).await;
    let entry_server = &data.0;
    //check entry server health
    let is_healthy = entry_server
        .is_healthy()
        .await
        .expect("Health check failed");
    assert!(is_healthy, "Entry server should be healthy");
}

/// Tests creating an entry through the gRPC service
#[rstest]
#[tokio_shared_rt::test]
async fn test_create_entry() {
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
async fn test_delete_entry() {
    use tonic::Request;
    use udex_api::entry::{
        ContextInput, CreateEntryRequest, DeleteEntryRequest, KeyValuePair, Value,
    };

    let data = data(false).await;
    let entry_server = &data.0;
    let index_name = &data.1;

    // First create an entry
    let context_input = ContextInput {
        pairs: vec![KeyValuePair {
            key: "test_key".to_string(),
            value: Some(Value {
                value: Some(udex_api::entry::value::Value::StringValue(
                    "test_value".to_string(),
                )),
            }),
            kek_id: None,
            dek: None,
        }],
    };

    let create_request = Request::new(CreateEntryRequest {
        index_name: index_name.clone(),
        context: Some(context_input),
    });

    let create_response = entry_server
        .create_entry(create_request)
        .await
        .expect("Create entry failed");
    let key = create_response.into_inner().key;

    // Now delete the entry
    let delete_request = Request::new(DeleteEntryRequest {
        index_name: index_name.clone(),
        key: key.clone(),
    });
    let _delete_response = entry_server
        .delete_entry(delete_request)
        .await
        .expect("Delete entry failed");

    // Verify entry is deleted by trying to lookup
    let lookup_request = Request::new(udex_api::entry::LookupContextByKeyRequest {
        index_name: index_name.clone(),
        key,
    });
    let lookup_result = entry_server.lookup_context_by_key(lookup_request).await;
    assert!(
        lookup_result.is_err(),
        "Entry should be deleted and not found"
    );
}

/// Tests looking up context by key through the gRPC service
#[rstest]
#[tokio_shared_rt::test]
async fn test_lookup_context_by_key() {
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

/// Tests looking up the key by context hash through the gRPC service.
///
/// Under the 1:1 model, creating twice with the same context is idempotent — both
/// calls return the same key. A subsequent lookup returns that single key.
#[rstest]
#[tokio_shared_rt::test]
async fn test_lookup_key_by_context() {
    use tonic::Request;
    use udex_api::entry::{
        ContextInput, CreateEntryRequest, KeyValuePair, LookupKeyByContextRequest, Value,
    };

    let data = data(false).await;
    let entry_server = &data.0;
    let index_name = &data.1;

    let context_input = ContextInput {
        pairs: vec![KeyValuePair {
            key: "shared_context".to_string(),
            value: Some(Value {
                value: Some(udex_api::entry::value::Value::StringValue(
                    "shared_value".to_string(),
                )),
            }),
            kek_id: None,
            dek: None,
        }],
    };

    // First create — establishes the entry.
    let resp1 = entry_server
        .create_entry(Request::new(CreateEntryRequest {
            index_name: index_name.clone(),
            context: Some(context_input.clone()),
        }))
        .await
        .expect("Create entry 1 failed")
        .into_inner();
    let key1 = resp1.key;
    let context_hash = resp1.context_hash;

    // Second create with the same context — idempotent, must return the same key.
    let resp2 = entry_server
        .create_entry(Request::new(CreateEntryRequest {
            index_name: index_name.clone(),
            context: Some(context_input),
        }))
        .await
        .expect("Create entry 2 failed")
        .into_inner();
    assert_eq!(
        resp2.key, key1,
        "Idempotent create must return the same key"
    );

    // Lookup by context hash — returns the single stored key.
    let lookup = entry_server
        .lookup_key_by_context(Request::new(LookupKeyByContextRequest {
            index_name: index_name.clone(),
            context_hash,
        }))
        .await
        .expect("Lookup keys failed")
        .into_inner();

    let returned_key = lookup.key.expect("Expected a key for known context");
    assert_eq!(returned_key, key1, "Lookup must return the stored key");
    let _uuid = Uuid::parse_str(&returned_key).expect("Returned key should be a valid UUID");
}

/// Tests bulk write operations through the gRPC service
#[rstest]
#[tokio_shared_rt::test]
async fn test_bulk_write_entry_operation() {
    use tonic::Request;
    use udex_api::entry::{
        bulk_write_entry_operation::Operation, BulkWriteEntryOperation,
        BulkWriteEntryOperationRequest, ContextInput, KeyValuePair, Value,
    };

    let data = data(false).await;
    let entry_server = &data.0;
    let index_name = &data.1;

    // Create bulk write operations
    let operations = vec![
        BulkWriteEntryOperation {
            operation: Some(Operation::CreateEntry(
                udex_api::entry::CreateEntryRequest {
                    index_name: index_name.clone(),
                    context: Some(ContextInput {
                        pairs: vec![KeyValuePair {
                            key: "bulk_test1".to_string(),
                            value: Some(Value {
                                value: Some(udex_api::entry::value::Value::StringValue(
                                    "value1".to_string(),
                                )),
                            }),
                            kek_id: None,
                            dek: None,
                        }],
                    }),
                },
            )),
        },
        BulkWriteEntryOperation {
            operation: Some(Operation::CreateEntry(
                udex_api::entry::CreateEntryRequest {
                    index_name: index_name.clone(),
                    context: Some(ContextInput {
                        pairs: vec![KeyValuePair {
                            key: "bulk_test2".to_string(),
                            value: Some(Value {
                                value: Some(udex_api::entry::value::Value::StringValue(
                                    "value2".to_string(),
                                )),
                            }),
                            kek_id: None,
                            dek: None,
                        }],
                    }),
                },
            )),
        },
    ];

    let bulk_request = Request::new(BulkWriteEntryOperationRequest {
        index_name: index_name.clone(),
        operations,
    });
    let bulk_response = entry_server
        .bulk_write_entry_operation(bulk_request)
        .await
        .expect("Bulk write failed");
    let results = bulk_response.into_inner().results;

    // Verify we got results for both operations
    assert_eq!(results.len(), 2, "Should have 2 results");

    // Verify each result contains a valid key
    for result in results {
        if let Some(udex_api::entry::bulk_write_entry_operation_result::Result::CreateEntry(
            create_response,
        )) = result.result
        {
            assert!(!create_response.key.is_empty(), "Key should not be empty");
            let _uuid = Uuid::parse_str(&create_response.key).expect("Key should be valid UUID");
        } else {
            panic!("Expected create entry result");
        }
    }
}

/// Tests bulk read operations through the gRPC service
#[rstest]
#[tokio_shared_rt::test]
async fn test_bulk_read_entry_operation() {
    use tonic::Request;
    use udex_api::entry::{
        bulk_read_entry_operation::Operation, BulkReadEntryOperation,
        BulkReadEntryOperationRequest, ContextInput, CreateEntryRequest, KeyValuePair, Value,
    };

    let data = data(false).await;
    let entry_server = &data.0;
    let index_name = &data.1;

    // First create some entries
    let context_input = ContextInput {
        pairs: vec![KeyValuePair {
            key: "bulk_read_test".to_string(),
            value: Some(Value {
                value: Some(udex_api::entry::value::Value::StringValue(
                    "bulk_read_value".to_string(),
                )),
            }),
            kek_id: None,
            dek: None,
        }],
    };

    let create_request = Request::new(CreateEntryRequest {
        index_name: index_name.clone(),
        context: Some(context_input),
    });
    let create_response = entry_server
        .create_entry(create_request)
        .await
        .expect("Create entry failed");
    let create_result = create_response.into_inner();
    let key = create_result.key;
    let context_hash = create_result.context_hash;

    // Create bulk read operations
    let operations = vec![
        BulkReadEntryOperation {
            operation: Some(Operation::LookupContext(
                udex_api::entry::LookupContextByKeyRequest {
                    index_name: index_name.clone(),
                    key: key.clone(),
                },
            )),
        },
        BulkReadEntryOperation {
            operation: Some(Operation::LookupKey(
                udex_api::entry::LookupKeyByContextRequest {
                    index_name: index_name.clone(),
                    context_hash,
                },
            )),
        },
    ];

    let bulk_request = Request::new(BulkReadEntryOperationRequest {
        index_name: index_name.clone(),
        operations,
    });
    let bulk_response = entry_server
        .bulk_read_entry_operation(bulk_request)
        .await
        .expect("Bulk read failed");
    let results = bulk_response.into_inner().results;

    // Verify we got results for both operations
    assert_eq!(results.len(), 2, "Should have 2 results");

    // First result should be a context lookup
    if let Some(udex_api::entry::bulk_read_entry_operation_result::Result::LookupContext(
        context_response,
    )) = &results[0].result
    {
        assert!(
            context_response.context.is_some(),
            "Context should be present"
        );
    } else {
        panic!("Expected lookup context result");
    }

    // Second result should be a keys lookup returning the single stored key.
    if let Some(udex_api::entry::bulk_read_entry_operation_result::Result::LookupKey(
        keys_response,
    )) = &results[1].result
    {
        let returned_key = keys_response
            .key
            .as_deref()
            .expect("Expected a key for known context");
        assert_eq!(returned_key, key, "Lookup must return the stored key");
    } else {
        panic!("Expected lookup keys result");
    }
}

/// An empty BulkWriteEntryOperationRequest is rejected with INVALID_ARGUMENT.
#[rstest]
#[tokio_shared_rt::test]
async fn test_bulk_write_empty_operations_invalid_argument() {
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
async fn test_bulk_read_empty_operations_invalid_argument() {
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

/// Verifies that the context hash is computed from (key, value) only — kek_id and dek on a
/// pair are excluded. Two creates with the same (key, value) but different per-pair dek/kek_id
/// resolve to the same context hash and return the first entry's key.
/// Callers that need to read back the stored encryption metadata should use
/// `lookup_context_by_key` and inspect the pairs.
#[rstest]
#[tokio_shared_rt::test]
async fn test_create_entry_idempotent_for_same_pairs_different_dek() {
    use tonic::Request;
    use udex_api::entry::{ContextInput, CreateEntryRequest, KeyValuePair, Value};

    let data = data(false).await;
    let entry_server = &data.0;
    let index_name = &data.1;

    let ciphertext = "encrypted_user_id_ciphertext";

    // First create: encrypted pair with dek_v1.
    let first = entry_server
        .create_entry(Request::new(CreateEntryRequest {
            index_name: index_name.clone(),
            context: Some(ContextInput {
                pairs: vec![KeyValuePair {
                    key: "user_id".to_string(),
                    value: Some(Value {
                        value: Some(udex_api::entry::value::Value::StringValue(
                            ciphertext.to_string(),
                        )),
                    }),
                    kek_id: Some("kek_001".to_string()),
                    dek: Some("dek_v1".to_string()),
                }],
            }),
        }))
        .await
        .expect("first create_entry should succeed")
        .into_inner();
    let key1 = first.key;

    // Second create: same (key, value) but different dek — same hash, same entry returned.
    let second = entry_server
        .create_entry(Request::new(CreateEntryRequest {
            index_name: index_name.clone(),
            context: Some(ContextInput {
                pairs: vec![KeyValuePair {
                    key: "user_id".to_string(),
                    value: Some(Value {
                        value: Some(udex_api::entry::value::Value::StringValue(
                            ciphertext.to_string(),
                        )),
                    }),
                    kek_id: Some("kek_001".to_string()),
                    dek: Some("dek_v2".to_string()),
                }],
            }),
        }))
        .await
        .expect("second create_entry should succeed (idempotent)")
        .into_inner();

    assert_eq!(
        second.key, key1,
        "Same (key, value) with different dek must return the same entry key"
    );

    // The stored pair carries dek_v1 (the first writer wins).
    let stored = entry_server
        .lookup_context_by_key(Request::new(udex_api::entry::LookupContextByKeyRequest {
            index_name: index_name.clone(),
            key: key1.clone(),
        }))
        .await
        .expect("lookup_context_by_key should succeed")
        .into_inner()
        .context
        .expect("context must be present");

    assert_eq!(
        stored.pairs[0].dek.as_deref(),
        Some("dek_v1"),
        "First-writer dek must win; second create must not overwrite it"
    );
}

/// Tests error handling for invalid operations
#[rstest]
#[tokio_shared_rt::test]
async fn test_error_handling() {
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
async fn test_lookup_or_create_creates_new_entry() {
    use tonic::Request;
    use udex_api::entry::LookupKeyByContextOrCreateRequest;

    let data = data(false).await;
    let entry_server = &data.0;
    let index_name = &data.1;

    let (ctx, hash) = loc_context("loc_create_user");

    let resp = entry_server
        .lookup_key_by_context_or_create(Request::new(LookupKeyByContextOrCreateRequest {
            index_name: index_name.clone(),
            context: Some(ctx),
            context_hash: hash.clone(),
        }))
        .await
        .expect("lookup_or_create on new context should succeed")
        .into_inner();

    assert!(resp.created, "created must be true for a new entry");
    assert!(!resp.key.is_empty(), "key must not be empty");
    assert_eq!(
        resp.context_hash, hash,
        "response hash must echo the request hash"
    );
    Uuid::parse_str(&resp.key).expect("key must be a valid UUID");
}

/// lookup_key_by_context_or_create: second call with the same context returns
/// the existing key unchanged and created=false.
#[rstest]
#[tokio_shared_rt::test]
async fn test_lookup_or_create_returns_existing_entry() {
    use tonic::Request;
    use udex_api::entry::LookupKeyByContextOrCreateRequest;

    let data = data(false).await;
    let entry_server = &data.0;
    let index_name = &data.1;

    let (ctx, hash) = loc_context("loc_found_user");

    let first = entry_server
        .lookup_key_by_context_or_create(Request::new(LookupKeyByContextOrCreateRequest {
            index_name: index_name.clone(),
            context: Some(ctx.clone()),
            context_hash: hash.clone(),
        }))
        .await
        .expect("first lookup_or_create should succeed")
        .into_inner();
    assert!(first.created, "first call must create the entry");

    let second = entry_server
        .lookup_key_by_context_or_create(Request::new(LookupKeyByContextOrCreateRequest {
            index_name: index_name.clone(),
            context: Some(ctx),
            context_hash: hash,
        }))
        .await
        .expect("second lookup_or_create should succeed")
        .into_inner();

    assert!(!second.created, "second call must not create a new entry");
    assert_eq!(
        second.key, first.key,
        "second call must return the same key as the first"
    );
}

/// lookup_key_by_context_or_create: missing context field returns INVALID_ARGUMENT.
/// Missing context_hash field returns INVALID_ARGUMENT.
/// Empty index_name returns INVALID_ARGUMENT.
#[rstest]
#[tokio_shared_rt::test]
async fn test_lookup_or_create_validation_errors() {
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

/// lookup_key_by_context_or_create: a context_hash that does not match the
/// server-computed hash returns INVALID_ARGUMENT immediately, before any
/// database access — even when the entry does not exist.
#[rstest]
#[tokio_shared_rt::test]
async fn test_lookup_or_create_hash_mismatch_invalid_argument() {
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

/// lookup_or_create inside a bulk write: verifies created=true on first call
/// and that the LookupOrCreate result variant is populated in the bulk response.
#[rstest]
#[tokio_shared_rt::test]
async fn test_bulk_write_lookup_or_create() {
    use tonic::Request;
    use udex_api::entry::{
        bulk_write_entry_operation::Operation,
        bulk_write_entry_operation_result::Result as WriteResult, BulkWriteEntryOperation,
        BulkWriteEntryOperationRequest, LookupKeyByContextOrCreateRequest,
    };

    let data = data(false).await;
    let entry_server = &data.0;
    let index_name = &data.1;

    let (ctx, hash) = loc_context("loc_bulk_user");

    let resp = entry_server
        .bulk_write_entry_operation(Request::new(BulkWriteEntryOperationRequest {
            index_name: index_name.clone(),
            operations: vec![BulkWriteEntryOperation {
                operation: Some(Operation::LookupOrCreate(
                    LookupKeyByContextOrCreateRequest {
                        index_name: index_name.clone(),
                        context: Some(ctx),
                        context_hash: hash.clone(),
                    },
                )),
            }],
        }))
        .await
        .expect("bulk write with lookup_or_create should succeed")
        .into_inner();

    assert_eq!(resp.results.len(), 1, "should have one result");

    match resp.results[0].result.as_ref().expect("result must be set") {
        WriteResult::LookupOrCreate(r) => {
            assert!(r.created, "created must be true for a new entry");
            assert!(!r.key.is_empty(), "key must not be empty");
            assert_eq!(
                r.context_hash, hash,
                "response hash must echo the request hash"
            );
            Uuid::parse_str(&r.key).expect("key must be a valid UUID");
        }
        other => panic!("expected LookupOrCreate result, got {:?}", other),
    }
}
