use crate::entry::ContextInput;
use crate::Error;

pub type ContextHasher = fn(&ContextInput) -> Result<String, Error>;

/// Hashes a context using xxHash (xxh3-64).
///
/// Pairs are sorted by key before serialisation so that submission order does
/// not affect the hash — two `ContextInput` values with the same pairs in any
/// order will always produce the same hash.
pub fn xxh3_context_hash(context: &ContextInput) -> Result<String, Error> {
    use xxhash_rust::xxh3::xxh3_64;

    let mut sorted_pairs = context.pairs.clone();
    sorted_pairs.sort_by(|a, b| a.key.cmp(&b.key));

    let context_json =
        serde_json::to_string(&sorted_pairs).map_err(|e| Error::DataConversion(e.to_string()))?;

    Ok(format!("{:016x}", xxh3_64(context_json.as_bytes())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::{value::Value as ValueOneof, KeyValuePair, Value};

    fn create_string_value(s: &str) -> Option<Value> {
        Some(Value {
            value: Some(ValueOneof::StringValue(s.to_string())),
        })
    }

    fn create_int_value(i: i64) -> Option<Value> {
        Some(Value {
            value: Some(ValueOneof::IntValue(i)),
        })
    }

    fn create_float_value(f: f64) -> Option<Value> {
        Some(Value {
            value: Some(ValueOneof::FloatValue(f)),
        })
    }

    fn create_bool_value(b: bool) -> Option<Value> {
        Some(Value {
            value: Some(ValueOneof::BoolValue(b)),
        })
    }

    #[test]
    fn test_same_context_same_hash() {
        let context1 = ContextInput {
            pairs: vec![
                KeyValuePair {
                    key: "name".to_string(),
                    value: create_string_value("Alice"),
                    kek_id: None,
                },
                KeyValuePair {
                    key: "age".to_string(),
                    value: create_int_value(30),
                    kek_id: None,
                },
            ],
            dek: None,
            kek_id: None,
        };

        let context2 = ContextInput {
            pairs: vec![
                KeyValuePair {
                    key: "name".to_string(),
                    value: create_string_value("Alice"),
                    kek_id: None,
                },
                KeyValuePair {
                    key: "age".to_string(),
                    value: create_int_value(30),
                    kek_id: None,
                },
            ],
            dek: None,
            kek_id: None,
        };

        let hash1 = xxh3_context_hash(&context1).expect("Hash should succeed");
        let hash2 = xxh3_context_hash(&context2).expect("Hash should succeed");

        assert_eq!(
            hash1, hash2,
            "Identical contexts should produce the same hash"
        );
    }

    #[test]
    fn test_different_key_order_same_hash() {
        let context1 = ContextInput {
            pairs: vec![
                KeyValuePair {
                    key: "name".to_string(),
                    value: create_string_value("Alice"),
                    kek_id: None,
                },
                KeyValuePair {
                    key: "age".to_string(),
                    value: create_int_value(30),
                    kek_id: None,
                },
                KeyValuePair {
                    key: "city".to_string(),
                    value: create_string_value("New York"),
                    kek_id: None,
                },
            ],
            dek: None,
            kek_id: None,
        };

        let context2 = ContextInput {
            pairs: vec![
                KeyValuePair {
                    key: "city".to_string(),
                    value: create_string_value("New York"),
                    kek_id: None,
                },
                KeyValuePair {
                    key: "name".to_string(),
                    value: create_string_value("Alice"),
                    kek_id: None,
                },
                KeyValuePair {
                    key: "age".to_string(),
                    value: create_int_value(30),
                    kek_id: None,
                },
            ],
            dek: None,
            kek_id: None,
        };

        let hash1 = xxh3_context_hash(&context1).expect("Hash should succeed");
        let hash2 = xxh3_context_hash(&context2).expect("Hash should succeed");

        assert_eq!(
            hash1, hash2,
            "Same key-value pairs in different order should produce the same hash"
        );
    }

    #[test]
    fn test_different_contexts_different_hashes() {
        let context1 = ContextInput {
            pairs: vec![
                KeyValuePair {
                    key: "name".to_string(),
                    value: create_string_value("Alice"),
                    kek_id: None,
                },
                KeyValuePair {
                    key: "age".to_string(),
                    value: create_int_value(30),
                    kek_id: None,
                },
            ],
            dek: None,
            kek_id: None,
        };

        let context2 = ContextInput {
            pairs: vec![
                KeyValuePair {
                    key: "name".to_string(),
                    value: create_string_value("Bob"),
                    kek_id: None,
                },
                KeyValuePair {
                    key: "age".to_string(),
                    value: create_int_value(30),
                    kek_id: None,
                },
            ],
            dek: None,
            kek_id: None,
        };

        let hash1 = xxh3_context_hash(&context1).expect("Hash should succeed");
        let hash2 = xxh3_context_hash(&context2).expect("Hash should succeed");

        assert_ne!(
            hash1, hash2,
            "Different contexts should produce different hashes"
        );
    }

    #[test]
    fn test_different_values_different_hashes() {
        let context1 = ContextInput {
            pairs: vec![KeyValuePair {
                key: "age".to_string(),
                value: create_int_value(30),
                kek_id: None,
            }],
            dek: None,
            kek_id: None,
        };

        let context2 = ContextInput {
            pairs: vec![KeyValuePair {
                key: "age".to_string(),
                value: create_int_value(31),
                kek_id: None,
            }],
            dek: None,
            kek_id: None,
        };

        let hash1 = xxh3_context_hash(&context1).expect("Hash should succeed");
        let hash2 = xxh3_context_hash(&context2).expect("Hash should succeed");

        assert_ne!(
            hash1, hash2,
            "Different values should produce different hashes"
        );
    }

    #[test]
    fn test_different_keys_different_hashes() {
        let context1 = ContextInput {
            pairs: vec![KeyValuePair {
                key: "name".to_string(),
                value: create_string_value("Alice"),
                kek_id: None,
            }],
            dek: None,
            kek_id: None,
        };

        let context2 = ContextInput {
            pairs: vec![KeyValuePair {
                key: "username".to_string(),
                value: create_string_value("Alice"),
                kek_id: None,
            }],
            dek: None,
            kek_id: None,
        };

        let hash1 = xxh3_context_hash(&context1).expect("Hash should succeed");
        let hash2 = xxh3_context_hash(&context2).expect("Hash should succeed");

        assert_ne!(
            hash1, hash2,
            "Different keys should produce different hashes"
        );
    }

    #[test]
    fn test_different_value_types_different_hashes() {
        let context1 = ContextInput {
            pairs: vec![KeyValuePair {
                key: "value".to_string(),
                value: create_string_value("42"),
                kek_id: None,
            }],
            dek: None,
            kek_id: None,
        };

        let context2 = ContextInput {
            pairs: vec![KeyValuePair {
                key: "value".to_string(),
                value: create_int_value(42),
                kek_id: None,
            }],
            dek: None,
            kek_id: None,
        };

        let hash1 = xxh3_context_hash(&context1).expect("Hash should succeed");
        let hash2 = xxh3_context_hash(&context2).expect("Hash should succeed");

        assert_ne!(
            hash1, hash2,
            "Same value but different types should produce different hashes"
        );
    }

    #[test]
    fn test_empty_context_consistent_hash() {
        let context1 = ContextInput {
            pairs: vec![],
            dek: None,
            kek_id: None,
        };

        let context2 = ContextInput {
            pairs: vec![],
            dek: None,
            kek_id: None,
        };

        let hash1 = xxh3_context_hash(&context1).expect("Hash should succeed");
        let hash2 = xxh3_context_hash(&context2).expect("Hash should succeed");

        assert_eq!(hash1, hash2, "Empty contexts should produce the same hash");
    }

    #[test]
    fn test_complex_reordering_same_hash() {
        let context1 = ContextInput {
            pairs: vec![
                KeyValuePair {
                    key: "z_last".to_string(),
                    value: create_string_value("last"),
                    kek_id: None,
                },
                KeyValuePair {
                    key: "a_first".to_string(),
                    value: create_string_value("first"),
                    kek_id: None,
                },
                KeyValuePair {
                    key: "m_middle".to_string(),
                    value: create_int_value(42),
                    kek_id: None,
                },
                KeyValuePair {
                    key: "b_second".to_string(),
                    value: create_bool_value(true),
                    kek_id: None,
                },
                KeyValuePair {
                    key: "x_float".to_string(),
                    value: create_float_value(std::f64::consts::PI),
                    kek_id: None,
                },
            ],
            dek: None,
            kek_id: None,
        };

        let context2 = ContextInput {
            pairs: vec![
                KeyValuePair {
                    key: "x_float".to_string(),
                    value: create_float_value(std::f64::consts::PI),
                    kek_id: None,
                },
                KeyValuePair {
                    key: "b_second".to_string(),
                    value: create_bool_value(true),
                    kek_id: None,
                },
                KeyValuePair {
                    key: "a_first".to_string(),
                    value: create_string_value("first"),
                    kek_id: None,
                },
                KeyValuePair {
                    key: "z_last".to_string(),
                    value: create_string_value("last"),
                    kek_id: None,
                },
                KeyValuePair {
                    key: "m_middle".to_string(),
                    value: create_int_value(42),
                    kek_id: None,
                },
            ],
            dek: None,
            kek_id: None,
        };

        let hash1 = xxh3_context_hash(&context1).expect("Hash should succeed");
        let hash2 = xxh3_context_hash(&context2).expect("Hash should succeed");

        assert_eq!(
            hash1, hash2,
            "Complex reordering of same key-value pairs should produce the same hash"
        );
    }

    #[test]
    fn test_hash_deterministic() {
        // This test verifies that the hash function is deterministic, meaning that
        // given the same input, it will always produce the same output across multiple calls.
        // This is a critical property for a hash function used in a lookup system because:
        // 1. It ensures that the same context will always map to the same hash
        // 2. It guarantees consistent behavior across different runs of the application
        // 3. It enables reliable storage and retrieval of contexts by their hash
        //
        // Without determinism, contexts could not be reliably found after being stored,
        // as their hash might change between storage and retrieval operations.

        let context = ContextInput {
            pairs: vec![KeyValuePair {
                key: "test".to_string(),
                value: create_string_value("value"),
                kek_id: None,
            }],
            dek: None,
            kek_id: None,
        };

        // Generate the hash three times from the same input
        let hash1 = xxh3_context_hash(&context).expect("Hash should succeed");
        let hash2 = xxh3_context_hash(&context).expect("Hash should succeed");
        let hash3 = xxh3_context_hash(&context).expect("Hash should succeed");

        // All three hashes must be identical - this proves determinism
        assert_eq!(
            hash1, hash2,
            "Hash should be deterministic - same input must always produce same output"
        );
        assert_eq!(
            hash2, hash3,
            "Hash should be deterministic - same input must always produce same output"
        );
        assert_eq!(
            hash1, hash3,
            "Hash should be deterministic - same input must always produce same output"
        );
    }

    #[test]
    fn test_hash_format() {
        let context = ContextInput {
            pairs: vec![KeyValuePair {
                key: "test".to_string(),
                value: create_string_value("value"),
                kek_id: None,
            }],
            dek: None,
            kek_id: None,
        };

        let hash = xxh3_context_hash(&context).expect("Hash should succeed");

        // xxh3-64 produces a 64-bit value — 16 hex characters
        assert_eq!(hash.len(), 16, "xxh3-64 hash should be 16 hex characters");

        // Should only contain hex characters
        assert!(
            hash.chars().all(|c| c.is_ascii_hexdigit()),
            "Hash should only contain hex characters"
        );

        // Should be lowercase
        assert_eq!(hash, hash.to_lowercase(), "Hash should be lowercase");
    }
}
