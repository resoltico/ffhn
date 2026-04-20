use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::CoreError;

/// Serializes one value using FFHN's stable compact JSON rendering.
pub fn stable_json<T: Serialize>(value: &T) -> Result<String, CoreError> {
    let value = serde_json::to_value(value)?;
    let mut output = String::new();
    write_stable_json_value(&value, &mut output)?;
    Ok(output)
}

/// Computes the SHA-256 digest of one value after stable serialization with one field omitted.
pub fn stable_digest_omitting_field<T: Serialize>(
    value: &T,
    field: &str,
) -> Result<String, CoreError> {
    let mut value = serde_json::to_value(value)?;
    if let Value::Object(map) = &mut value {
        map.remove(field);
    }

    let mut output = String::new();
    write_stable_json_value(&value, &mut output)?;
    Ok(sha256_hex(output.as_bytes()))
}

/// Computes the SHA-256 digest for one byte slice.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

fn write_stable_json_value(value: &Value, output: &mut String) -> Result<(), CoreError> {
    match value {
        Value::Null => output.push_str("null"),
        Value::Bool(value) => output.push_str(if *value { "true" } else { "false" }),
        Value::Number(value) => output.push_str(&value.to_string()),
        Value::String(value) => output.push_str(&serde_json::to_string(value)?),
        Value::Array(values) => {
            output.push('[');
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                write_stable_json_value(value, output)?;
            }
            output.push(']');
        }
        Value::Object(map) => {
            output.push('{');
            let mut entries = map.iter().collect::<Vec<_>>();
            entries.sort_unstable_by(|(left, _), (right, _)| left.cmp(right));
            for (index, (key, value)) in entries.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                output.push_str(&serde_json::to_string(key)?);
                output.push(':');
                write_stable_json_value(value, output)?;
            }
            output.push('}');
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;
    use serde_json::json;

    #[derive(Serialize)]
    struct Example<'a> {
        b: &'a str,
        a: &'a str,
        nested: Value,
    }

    #[test]
    fn stable_json_orders_object_keys_recursively() {
        let rendered = stable_json(&Example {
            b: "two",
            a: "one",
            nested: json!({
                "z": 2,
                "a": [3, {"d": true, "c": false}],
            }),
        })
        .expect("stable json");

        assert_eq!(
            rendered,
            r#"{"a":"one","b":"two","nested":{"a":[3,{"c":false,"d":true}],"z":2}}"#
        );
    }

    #[test]
    fn stable_digest_omitting_field_ignores_the_named_field() {
        let left = json!({
            "digest": "aaa",
            "payload": {"b": 2, "a": 1},
        });
        let right = json!({
            "digest": "bbb",
            "payload": {"a": 1, "b": 2},
        });

        let left_digest =
            stable_digest_omitting_field(&left, "digest").expect("left digest omission");
        let right_digest =
            stable_digest_omitting_field(&right, "digest").expect("right digest omission");

        assert_eq!(left_digest, right_digest);
    }

    #[test]
    fn sha256_hex_is_lowercase_hex() {
        assert_eq!(
            sha256_hex(b"ffhn"),
            "6cc78d9c44007646e3f2a5013ad77c58cfe84c2f8985e1900533086a917954d7"
        );
    }

    #[test]
    fn stable_json_handles_nulls_and_non_object_digest_omission() {
        assert_eq!(stable_json(&Value::Null).expect("null json"), "null");

        let digest = stable_digest_omitting_field(&json!(["a", null, {"b": true}]), "ignored")
            .expect("array digest");
        assert_eq!(digest.len(), 64);
    }
}
