//! JSON-to-SDK conversion utilities.

use udex_api::entry::{value, ContextInput, KeyValuePair, Value};

use crate::error::Error;

/// Converts a flat JSON object into a [`ContextInput`].
///
/// Each key-value pair in the JSON object becomes a context pair. The JSON
/// value type is mapped to the closest SDK type:
///
/// | JSON type | SDK [`Value`] variant |
/// |-----------|-----------------------|
/// | string    | [`StringValue`]       |
/// | integer   | [`IntValue`]          |
/// | float     | [`FloatValue`]        |
/// | boolean   | [`BoolValue`]         |
///
/// Objects, arrays, and `null` are rejected with [`Error::InvalidOptions`].
///
/// [`StringValue`]: value::Value::StringValue
/// [`IntValue`]: value::Value::IntValue
/// [`FloatValue`]: value::Value::FloatValue
/// [`BoolValue`]: value::Value::BoolValue
pub fn context_input_from_json(
    map: serde_json::Map<String, serde_json::Value>,
) -> Result<ContextInput, Error> {
    let pairs = map
        .into_iter()
        .map(|(k, v)| {
            let proto_value = json_value_to_proto(&k, v)?;
            Ok(KeyValuePair {
                key: k,
                value: Some(Value {
                    value: Some(proto_value),
                }),
                kek_id: None,
                dek: None,
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;

    Ok(ContextInput { pairs })
}

fn json_value_to_proto(key: &str, v: serde_json::Value) -> Result<value::Value, Error> {
    match v {
        serde_json::Value::String(s) => Ok(value::Value::StringValue(s)),
        serde_json::Value::Bool(b) => Ok(value::Value::BoolValue(b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(value::Value::IntValue(i))
            } else if let Some(f) = n.as_f64() {
                Ok(value::Value::FloatValue(f))
            } else {
                Err(Error::InvalidOptions(format!(
                    "key {key:?}: number {n:?} cannot be represented as a context value",
                )))
            }
        }
        other => Err(Error::InvalidOptions(format!(
            "key {key:?}: {ty} values cannot be used as context values",
            ty = json_type_name(&other),
        ))),
    }
}

fn json_type_name(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::Null => "null",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: &str) -> Result<ContextInput, Error> {
        let map: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(json).expect("valid JSON");
        context_input_from_json(map)
    }

    fn value_of(ctx: &ContextInput, key: &str) -> value::Value {
        ctx.pairs
            .iter()
            .find(|p| p.key == key)
            .and_then(|p| p.value.as_ref())
            .and_then(|v| v.value.clone())
            .unwrap_or_else(|| panic!("key {key:?} not found in context"))
    }

    #[test]
    fn string_value() {
        let ctx = parse(r#"{"name":"alice"}"#).unwrap();
        let value::Value::StringValue(s) = value_of(&ctx, "name") else {
            panic!("expected StringValue");
        };
        assert_eq!(s, "alice");
    }

    #[test]
    fn integer_value() {
        let ctx = parse(r#"{"count":42}"#).unwrap();
        assert_eq!(value_of(&ctx, "count"), value::Value::IntValue(42));
    }

    #[test]
    fn negative_integer() {
        let ctx = parse(r#"{"offset":-7}"#).unwrap();
        assert_eq!(value_of(&ctx, "offset"), value::Value::IntValue(-7));
    }

    #[test]
    fn float_value() {
        let ctx = parse(r#"{"score":1.5}"#).unwrap();
        let value::Value::FloatValue(f) = value_of(&ctx, "score") else {
            panic!("expected FloatValue");
        };
        assert!((f - 1.5).abs() < 1e-10, "unexpected float: {f}");
    }

    #[test]
    fn bool_true() {
        let ctx = parse(r#"{"active":true}"#).unwrap();
        assert_eq!(value_of(&ctx, "active"), value::Value::BoolValue(true));
    }

    #[test]
    fn bool_false() {
        let ctx = parse(r#"{"active":false}"#).unwrap();
        assert_eq!(value_of(&ctx, "active"), value::Value::BoolValue(false));
    }

    #[test]
    fn mixed_scalar_types() {
        let ctx = parse(r#"{"name":"bob","age":30,"score":9.5,"active":true}"#).unwrap();
        let value::Value::StringValue(s) = value_of(&ctx, "name") else {
            panic!("expected StringValue");
        };
        assert_eq!(s, "bob");
        assert_eq!(value_of(&ctx, "age"), value::Value::IntValue(30));
        assert_eq!(value_of(&ctx, "active"), value::Value::BoolValue(true));
        let value::Value::FloatValue(f) = value_of(&ctx, "score") else {
            panic!("expected FloatValue");
        };
        assert!((f - 9.5).abs() < 1e-10);
    }

    #[test]
    fn empty_object_produces_empty_context() {
        let ctx = parse("{}").unwrap();
        assert!(ctx.pairs.is_empty());
    }

    #[test]
    fn null_rejected() {
        let err = parse(r#"{"x":null}"#).unwrap_err();
        assert!(
            err.to_string().contains("null"),
            "expected 'null' in error: {err}"
        );
    }

    #[test]
    fn array_rejected() {
        let err = parse(r#"{"x":[1,2,3]}"#).unwrap_err();
        assert!(
            err.to_string().contains("array"),
            "expected 'array' in error: {err}"
        );
    }

    #[test]
    fn object_rejected() {
        let err = parse(r#"{"x":{"nested":true}}"#).unwrap_err();
        assert!(
            err.to_string().contains("object"),
            "expected 'object' in error: {err}"
        );
    }

    #[test]
    fn error_message_names_the_offending_key() {
        let err = parse(r#"{"good":"ok","bad":null}"#).unwrap_err();
        assert!(
            err.to_string().contains("bad"),
            "expected key name in error: {err}"
        );
    }
}
