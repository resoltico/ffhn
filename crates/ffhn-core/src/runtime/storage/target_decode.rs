//! FFHN-owned target TOML decoding with safe field-level failure evidence.

use serde_path_to_error::{Path, Segment};

use crate::{CoreError, TargetDecodeError, TargetDocument};

/// Decodes one target document while converting deserializer failures into FFHN-owned facts.
pub(super) fn decode_target(text: &str) -> Result<TargetDocument, CoreError> {
    let document: toml::Value = toml::from_str(text).map_err(|_| TargetDecodeError::Syntax)?;
    let deserializer =
        toml::de::Deserializer::parse(text).map_err(|_| TargetDecodeError::Syntax)?;
    serde_path_to_error::deserialize(deserializer).map_err(|error| {
        let path = error.path();
        let (field, received) = invalid_closed_vocabulary(&document)
            .unwrap_or_else(|| (path.to_string(), closed_vocabulary_value(&document, path)));
        TargetDecodeError::InvalidField { field, received }.into()
    })
}

/// Finds the one invalid safe enum value that Serde's tagged-enum error path can hide.
///
/// TOML reports a failed nested tagged enum at the containing table (for example,
/// `projection`), even when the bad input was `projection.selection.rendering.whitespace`.
/// This is diagnostic refinement only: normal `TargetDocument` deserialization remains the
/// authority for target validity. When more than one known field is invalid, preserving Serde's
/// path is more truthful than inventing an order between independent errors.
fn invalid_closed_vocabulary(document: &toml::Value) -> Option<(String, Option<String>)> {
    const VOCABULARIES: &[(&str, &[&str])] = &[
        (
            "declared_type",
            &["text", "integer", "decimal", "money", "semver", "datetime"],
        ),
        ("target.kind", &["http", "file"]),
        ("fetch.engine", &["http", "file"]),
        ("fetch.method", &["GET"]),
        (
            "projection.kind",
            &[
                "json_pointer",
                "html_text",
                "html_rendered_text",
                "html_attribute",
            ],
        ),
        (
            "projection.selection.strategy.kind",
            &["css_selector", "delimiter_pair"],
        ),
        (
            "projection.selection.selection.mode",
            &["single", "first", "nth", "all"],
        ),
        (
            "projection.selection.rendering.whitespace",
            &["rendered", "normalize"],
        ),
        ("type_params.locale", &["invariant", "en_us", "de_de"]),
    ];

    let invalid = VOCABULARIES
        .iter()
        .filter_map(|(field, allowed)| {
            let value = value_at_field(document, field)?.as_str()?;
            (!allowed.contains(&value)).then(|| ((*field).to_owned(), Some(rendered_string(value))))
        })
        .collect::<Vec<_>>();
    (invalid.len() == 1).then(|| invalid.into_iter().next().expect("one invalid vocabulary"))
}

/// Returns a value only for non-sensitive fields whose grammar is an FFHN-owned enum.
fn closed_vocabulary_value(document: &toml::Value, path: &Path) -> Option<String> {
    let field = path.to_string();
    if !is_closed_vocabulary_field(&field) {
        return None;
    }
    value_at_path(document, path).and_then(render_scalar)
}

fn value_at_field<'a>(mut value: &'a toml::Value, field: &str) -> Option<&'a toml::Value> {
    for segment in field.split('.') {
        value = value.get(segment)?;
    }
    Some(value)
}

fn is_closed_vocabulary_field(field: &str) -> bool {
    matches!(
        field,
        "declared_type"
            | "fetch.engine"
            | "fetch.method"
            | "projection.kind"
            | "projection.selection.strategy.kind"
            | "projection.selection.selection.mode"
            | "projection.selection.rendering.whitespace"
            | "type_params.locale"
    ) || field.ends_with(".predicate.kind")
        || field.ends_with(".predicate.reference")
        || field.ends_with(".route_family")
        || field.ends_with(".kind")
}

fn value_at_path<'value, 'path>(
    mut value: &'value toml::Value,
    path: impl IntoIterator<Item = &'path Segment>,
) -> Option<&'value toml::Value> {
    for segment in path {
        value = match segment {
            Segment::Map { key } | Segment::Enum { variant: key } => value.get(key)?,
            Segment::Seq { index } => value.as_array()?.get(*index)?,
            Segment::Unknown => return None,
        };
    }
    Some(value)
}

fn render_scalar(value: &toml::Value) -> Option<String> {
    match value {
        toml::Value::String(value) => Some(rendered_string(value)),
        toml::Value::Integer(value) => Some(value.to_string()),
        toml::Value::Float(value) => Some(value.to_string()),
        toml::Value::Boolean(value) => Some(value.to_string()),
        toml::Value::Datetime(value) => Some(value.to_string()),
        toml::Value::Array(_) | toml::Value::Table(_) => None,
    }
}

fn rendered_string(value: &str) -> String {
    serde_json::to_string(value).expect("a Rust string always serializes as JSON")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_value(source: &str) -> toml::Value {
        toml::from_str(source).expect("test TOML")
    }

    #[test]
    fn invalid_whitespace_identifies_the_field_and_the_closed_value() {
        let error = decode_target(
            "schema_name = \"ffhn.target\"\nschema_version = 12\ntarget_id = \"decode-test\"\ndisplay_name = \"Decode test\"\nenabled = true\nescalate_after = 2\ndeclared_type = \"text\"\nconditions = []\n\n[target]\nkind = \"file\"\nfile_path = \"/tmp/source.html\"\n\n[fetch]\nengine = \"file\"\n\n[projection]\nkind = \"html_text\"\n\n[projection.selection.strategy]\nkind = \"css_selector\"\nselector = \"h1\"\n\n[projection.selection.selection]\nmode = \"single\"\n\n[projection.selection.rendering]\nwhitespace = \"totally-bogus\"\nrewrite_urls = false\n",
        )
        .expect_err("invalid whitespace enum");
        assert_eq!(
            error.to_string(),
            "target decode error: target field \"projection.selection.rendering.whitespace\" could not be decoded"
        );
    }

    #[test]
    fn decoder_distinguishes_syntax_and_non_vocabulary_decode_failures() {
        let syntax = decode_target("not valid = [").expect_err("invalid TOML syntax");
        assert!(matches!(
            syntax,
            CoreError::TargetDecode(TargetDecodeError::Syntax)
        ));

        let non_vocabulary =
            decode_target("schema_name = \"ffhn.target\"\nschema_version = 12\ntarget_id = 7\n")
                .expect_err("target id must remain a string");
        assert_eq!(
            non_vocabulary.to_string(),
            "target decode error: target field \"target_id\" could not be decoded"
        );

        let closed_vocabulary = decode_target(
            "schema_name = \"ffhn.target\"\nschema_version = 12\ntarget_id = \"decode-test\"\ndisplay_name = \"Decode test\"\nenabled = true\nescalate_after = 2\ndeclared_type = 7\nconditions = []\n",
        )
        .expect_err("declared type must remain a closed string vocabulary");
        assert_eq!(
            closed_vocabulary.to_string(),
            "target decode error: target field \"declared_type\" could not be decoded"
        );
    }

    #[test]
    fn safe_vocabulary_refinement_is_unique_and_the_complete_closed_vocabulary_is_explicit() {
        let clean = parse_value("declared_type = \"text\"");
        assert_eq!(invalid_closed_vocabulary(&clean), None);

        let conflicting =
            parse_value("declared_type = \"unsupported\"\n[fetch]\nengine = \"other\"\n");
        assert_eq!(invalid_closed_vocabulary(&conflicting), None);

        for field in [
            "declared_type",
            "fetch.engine",
            "fetch.method",
            "projection.kind",
            "projection.selection.strategy.kind",
            "projection.selection.selection.mode",
            "projection.selection.rendering.whitespace",
            "type_params.locale",
            "conditions.0.predicate.kind",
            "conditions.0.predicate.reference",
            "routes.0.route_family",
            "target.kind",
        ] {
            assert!(is_closed_vocabulary_field(field), "{field}");
        }
        assert!(!is_closed_vocabulary_field("target_id"));
    }

    #[test]
    fn vocabulary_value_helpers_follow_maps_sequences_variants_and_safe_scalars() {
        let document = parse_value(
            "root = \"text\"\ninteger = 7\nfloat = 1.5\nboolean = true\ndatetime = 2026-07-19T00:00:00Z\narray = [\"item\"]\n[nested]\nvalue = \"nested\"\n",
        );
        assert_eq!(
            value_at_field(&document, "nested.value")
                .and_then(render_scalar)
                .as_deref(),
            Some("\"nested\"")
        );
        assert_eq!(value_at_field(&document, "nested.missing"), None);

        let map_root = Segment::Map {
            key: "root".to_owned(),
        };
        let enum_nested = Segment::Enum {
            variant: "nested".to_owned(),
        };
        let map_value = Segment::Map {
            key: "value".to_owned(),
        };
        let array = Segment::Map {
            key: "array".to_owned(),
        };
        let first = Segment::Seq { index: 0 };
        assert_eq!(
            value_at_path(&document, [&map_root])
                .and_then(render_scalar)
                .as_deref(),
            Some("\"text\"")
        );
        assert_eq!(
            value_at_path(&document, [&enum_nested, &map_value])
                .and_then(render_scalar)
                .as_deref(),
            Some("\"nested\"")
        );
        assert_eq!(
            value_at_path(&document, [&array, &first])
                .and_then(render_scalar)
                .as_deref(),
            Some("\"item\"")
        );
        assert_eq!(value_at_path(&document, [&Segment::Unknown]), None);

        for (field, expected) in [
            ("root", Some("\"text\"")),
            ("integer", Some("7")),
            ("float", Some("1.5")),
            ("boolean", Some("true")),
            ("datetime", Some("2026-07-19T00:00:00Z")),
            ("array", None),
            ("nested", None),
        ] {
            assert_eq!(
                value_at_field(&document, field)
                    .and_then(render_scalar)
                    .as_deref(),
                expected,
                "{field}"
            );
        }
    }
}
