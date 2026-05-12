use crate::entry::{ContextInput, Value};
use crate::Error;

pub type ContextHasher = fn(&ContextInput) -> Result<String, Error>;

/// Hashes a context using xxHash (xxh3-64).
///
/// Pairs are sorted by key before serialisation so that submission order does
/// not affect the hash — two `ContextInput` values with the same pairs in any
/// order will always produce the same hash.
///
/// Only `key` and `value` contribute to the hash. Encryption metadata (`kek_id`,
/// `dek`) is excluded: if the encrypted value changes (re-encryption, key
/// rotation) the new ciphertext already produces a different hash through the
/// value field, so there is no need to include the metadata separately.
pub fn xxh3_context_hash(context: &ContextInput) -> Result<String, Error> {
    use xxhash_rust::xxh3::xxh3_64;

    #[derive(serde::Serialize)]
    struct HashPair<'a> {
        key: &'a str,
        value: &'a Option<Value>,
    }

    let mut sorted_pairs: Vec<_> = context.pairs.iter().collect();
    sorted_pairs.sort_by(|a, b| a.key.cmp(&b.key));

    let hash_pairs: Vec<HashPair> = sorted_pairs
        .iter()
        .map(|p| HashPair {
            key: &p.key,
            value: &p.value,
        })
        .collect();

    let context_json =
        serde_json::to_string(&hash_pairs).map_err(|e| Error::DataConversion(e.to_string()))?;

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

    fn kv(key: &str, value: Option<Value>) -> KeyValuePair {
        KeyValuePair {
            key: key.to_string(),
            value,
            kek_id: None,
            dek: None,
        }
    }

    fn ctx(pairs: Vec<KeyValuePair>) -> ContextInput {
        ContextInput { pairs }
    }

    #[test]
    fn test_same_context_same_hash() {
        let context1 = ctx(vec![
            kv("name", create_string_value("Alice")),
            kv("age", create_int_value(30)),
        ]);
        let context2 = ctx(vec![
            kv("name", create_string_value("Alice")),
            kv("age", create_int_value(30)),
        ]);
        assert_eq!(
            xxh3_context_hash(&context1).unwrap(),
            xxh3_context_hash(&context2).unwrap(),
            "Identical contexts should produce the same hash"
        );
    }

    #[test]
    fn test_different_key_order_same_hash() {
        let context1 = ctx(vec![
            kv("name", create_string_value("Alice")),
            kv("age", create_int_value(30)),
            kv("city", create_string_value("New York")),
        ]);
        let context2 = ctx(vec![
            kv("city", create_string_value("New York")),
            kv("name", create_string_value("Alice")),
            kv("age", create_int_value(30)),
        ]);
        assert_eq!(
            xxh3_context_hash(&context1).unwrap(),
            xxh3_context_hash(&context2).unwrap(),
            "Same key-value pairs in different order should produce the same hash"
        );
    }

    #[test]
    fn test_different_contexts_different_hashes() {
        let context1 = ctx(vec![
            kv("name", create_string_value("Alice")),
            kv("age", create_int_value(30)),
        ]);
        let context2 = ctx(vec![
            kv("name", create_string_value("Bob")),
            kv("age", create_int_value(30)),
        ]);
        assert_ne!(
            xxh3_context_hash(&context1).unwrap(),
            xxh3_context_hash(&context2).unwrap(),
            "Different contexts should produce different hashes"
        );
    }

    #[test]
    fn test_different_values_different_hashes() {
        let c1 = ctx(vec![kv("age", create_int_value(30))]);
        let c2 = ctx(vec![kv("age", create_int_value(31))]);
        assert_ne!(
            xxh3_context_hash(&c1).unwrap(),
            xxh3_context_hash(&c2).unwrap()
        );
    }

    #[test]
    fn test_different_keys_different_hashes() {
        let c1 = ctx(vec![kv("name", create_string_value("Alice"))]);
        let c2 = ctx(vec![kv("username", create_string_value("Alice"))]);
        assert_ne!(
            xxh3_context_hash(&c1).unwrap(),
            xxh3_context_hash(&c2).unwrap()
        );
    }

    #[test]
    fn test_different_value_types_different_hashes() {
        let c1 = ctx(vec![kv("value", create_string_value("42"))]);
        let c2 = ctx(vec![kv("value", create_int_value(42))]);
        assert_ne!(
            xxh3_context_hash(&c1).unwrap(),
            xxh3_context_hash(&c2).unwrap(),
            "Same value but different types should produce different hashes"
        );
    }

    #[test]
    fn test_empty_context_consistent_hash() {
        assert_eq!(
            xxh3_context_hash(&ctx(vec![])).unwrap(),
            xxh3_context_hash(&ctx(vec![])).unwrap(),
            "Empty contexts should produce the same hash"
        );
    }

    #[test]
    fn test_complex_reordering_same_hash() {
        let context1 = ctx(vec![
            kv("z_last", create_string_value("last")),
            kv("a_first", create_string_value("first")),
            kv("m_middle", create_int_value(42)),
            kv("b_second", create_bool_value(true)),
            kv("x_float", create_float_value(std::f64::consts::PI)),
        ]);
        let context2 = ctx(vec![
            kv("x_float", create_float_value(std::f64::consts::PI)),
            kv("b_second", create_bool_value(true)),
            kv("a_first", create_string_value("first")),
            kv("z_last", create_string_value("last")),
            kv("m_middle", create_int_value(42)),
        ]);
        assert_eq!(
            xxh3_context_hash(&context1).unwrap(),
            xxh3_context_hash(&context2).unwrap(),
            "Complex reordering of same key-value pairs should produce the same hash"
        );
    }

    #[test]
    fn test_hash_deterministic() {
        let context = ctx(vec![kv("test", create_string_value("value"))]);
        let h1 = xxh3_context_hash(&context).unwrap();
        let h2 = xxh3_context_hash(&context).unwrap();
        let h3 = xxh3_context_hash(&context).unwrap();
        assert_eq!(h1, h2);
        assert_eq!(h2, h3);
    }

    #[test]
    fn test_hash_format() {
        let context = ctx(vec![kv("test", create_string_value("value"))]);
        let hash = xxh3_context_hash(&context).unwrap();
        assert_eq!(hash.len(), 16, "xxh3-64 hash should be 16 hex characters");
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(hash, hash.to_lowercase());
    }

    #[test]
    fn test_encryption_metadata_excluded_from_hash() {
        // kek_id and dek on a pair must not affect the hash — only key and value do.
        // This ensures DEK rotation or KEK relabelling doesn't create a new context entry.
        let plaintext = ctx(vec![kv("email", create_string_value("alice@example.com"))]);
        let with_metadata = ctx(vec![KeyValuePair {
            key: "email".to_string(),
            value: create_string_value("alice@example.com"),
            kek_id: Some("my-kek-v1".to_string()),
            dek: Some("wrapped-dek-bytes".to_string()),
        }]);
        assert_eq!(
            xxh3_context_hash(&plaintext).unwrap(),
            xxh3_context_hash(&with_metadata).unwrap(),
            "kek_id and dek must not affect the context hash"
        );
    }
}
