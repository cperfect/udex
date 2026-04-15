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
            max_bulk_operations: Some(100),
            max_key_length: Some(256),
            max_value_length: Some(1024),
            max_kv_pairs_per_context: Some(10),
            hash_algorithm: Some(HashAlgorithm::Sha1 as i32),
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
            },
            KeyValuePair {
                key: "session_id".to_string(),
                value: Some(Value {
                    value: Some(udex_api::entry::value::Value::StringValue(
                        "sess456".to_string(),
                    )),
                }),
                kek_id: None,
            },
        ],
        dek: None,
        kek_id: None,
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
        }],
        dek: None,
        kek_id: None,
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
        }],
        dek: None,
        kek_id: None,
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

/// Tests looking up keys by context hash through the gRPC service
#[rstest]
#[tokio_shared_rt::test]
async fn test_lookup_keys_by_context() {
    use tonic::Request;
    use udex_api::entry::{
        ContextInput, CreateEntryRequest, KeyValuePair, LookupKeysByContextRequest, Value,
    };

    let data = data(false).await;
    let entry_server = &data.0;
    let index_name = &data.1;

    // Create context input
    let context_input = ContextInput {
        pairs: vec![KeyValuePair {
            key: "shared_context".to_string(),
            value: Some(Value {
                value: Some(udex_api::entry::value::Value::StringValue(
                    "shared_value".to_string(),
                )),
            }),
            kek_id: None,
        }],
        dek: None,
        kek_id: None,
    };

    // Create first entry
    let create_request1 = Request::new(CreateEntryRequest {
        index_name: index_name.clone(),
        context: Some(context_input.clone()),
    });
    let create_response1 = entry_server
        .create_entry(create_request1)
        .await
        .expect("Create entry 1 failed");
    let context_hash = create_response1.into_inner().context_hash;

    // Create second entry with same context
    let create_request2 = Request::new(CreateEntryRequest {
        index_name: index_name.clone(),
        context: Some(context_input),
    });
    let _create_response2 = entry_server
        .create_entry(create_request2)
        .await
        .expect("Create entry 2 failed");

    // Lookup keys by context hash
    let lookup_request = Request::new(LookupKeysByContextRequest {
        index_name: index_name.clone(),
        context_hash: context_hash.clone(),
    });
    let lookup_response = entry_server
        .lookup_keys_by_context(lookup_request)
        .await
        .expect("Lookup keys failed");
    let keys = lookup_response.into_inner().keys;

    // Should have 2 keys for the same context
    assert_eq!(keys.len(), 2, "Should have 2 keys for the same context");

    // Verify all keys are valid UUIDs
    for key in &keys {
        let _uuid = Uuid::parse_str(key).expect("Each key should be valid UUID");
    }
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
                        }],
                        dek: None,
                        kek_id: None,
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
                        }],
                        dek: None,
                        kek_id: None,
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
        }],
        dek: None,
        kek_id: None,
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
            operation: Some(Operation::LookupKeys(
                udex_api::entry::LookupKeysByContextRequest {
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

    // Second result should be keys lookup
    if let Some(udex_api::entry::bulk_read_entry_operation_result::Result::LookupKeys(
        keys_response,
    )) = &results[1].result
    {
        assert!(!keys_response.keys.is_empty(), "Keys should not be empty");
        assert!(
            keys_response.keys.contains(&key),
            "Should contain our created key"
        );
    } else {
        panic!("Expected lookup keys result");
    }
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
