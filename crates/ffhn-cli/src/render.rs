//! Rendering of closed v11 graph documents.

use std::io::{self, Write};

use serde::Serialize;
use serde_json::Value;

use crate::args::OutputFormat;

/// Renders one v11 public document in the selected format.
pub(crate) fn render_document(
    stdout: &mut (impl Write + ?Sized),
    document: &impl Serialize,
    format: OutputFormat,
) -> io::Result<()> {
    match format {
        OutputFormat::Json => {
            serde_json::to_writer(&mut *stdout, document).map_err(io::Error::other)?;
            writeln!(stdout)
        }
        OutputFormat::JsonPretty => {
            serde_json::to_writer_pretty(&mut *stdout, document).map_err(io::Error::other)?;
            writeln!(stdout)
        }
        OutputFormat::Summary => {
            let value = serde_json::to_value(document).map_err(io::Error::other)?;
            render_summary_value(stdout, &value, 0)
        }
    }
}

fn render_summary_value(
    stdout: &mut (impl Write + ?Sized),
    value: &Value,
    depth: usize,
) -> io::Result<()> {
    match value {
        Value::Object(fields) => {
            for (name, value) in fields {
                let label = human_label(name);
                match value {
                    Value::Object(_) | Value::Array(_) => {
                        writeln!(stdout, "{}{}:", "  ".repeat(depth), label)?;
                        render_summary_value(stdout, value, depth + 1)?;
                    }
                    _ => writeln!(stdout, "{}{}: {}", "  ".repeat(depth), label, scalar(value))?,
                }
            }
            Ok(())
        }
        Value::Array(values) => {
            if values.is_empty() {
                return writeln!(stdout, "{}none", "  ".repeat(depth));
            }
            for value in values {
                match value {
                    Value::Object(_) | Value::Array(_) => {
                        writeln!(stdout, "{}-", "  ".repeat(depth))?;
                        render_summary_value(stdout, value, depth + 1)?;
                    }
                    _ => writeln!(stdout, "{}- {}", "  ".repeat(depth), scalar(value))?,
                }
            }
            Ok(())
        }
        _ => writeln!(stdout, "{}", scalar(value)),
    }
}

fn human_label(raw: &str) -> String {
    raw.split('_')
        .map(|word| {
            let mut letters = word.chars();
            match letters.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), letters.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn scalar(value: &Value) -> String {
    match value {
        Value::Null => "none".to_owned(),
        Value::String(value) => summary_string(value),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}

fn summary_string(value: &str) -> String {
    let mut rendered = String::with_capacity(value.len());
    for character in value.chars() {
        if character.is_control() {
            rendered.extend(character.escape_default());
        } else {
            rendered.push(character);
        }
    }
    rendered
}

#[cfg(test)]
mod tests {
    use serde::Serialize;

    use super::*;

    #[derive(Serialize)]
    struct Report {
        schema_name: &'static str,
        source_id: &'static str,
        checks: Vec<Check>,
    }

    #[derive(Serialize)]
    struct Check {
        valid: bool,
    }

    #[test]
    fn summary_is_human_text_and_not_pretty_json() {
        let mut output = Vec::new();
        render_document(
            &mut output,
            &Report {
                schema_name: "ffhn.validate_report",
                source_id: "shop",
                checks: vec![Check { valid: true }],
            },
            OutputFormat::Summary,
        )
        .expect("summary");
        let output = String::from_utf8(output).expect("UTF-8");
        assert!(output.contains("Schema Name: ffhn.validate_report"));
        assert!(output.contains("Checks:"));
        assert!(!output.contains('{'));
    }

    #[test]
    fn summary_escapes_control_characters_inside_string_facts() {
        #[derive(Serialize)]
        struct Diagnostic<'a> {
            message: &'a str,
        }
        let mut output = Vec::new();
        render_document(
            &mut output,
            &Diagnostic {
                message: "first\nsecond\tvalue",
            },
            OutputFormat::Summary,
        )
        .expect("summary");
        assert_eq!(
            String::from_utf8(output).expect("UTF-8"),
            "Message: first\\nsecond\\tvalue\n"
        );
    }

    #[test]
    fn summary_covers_empty_and_scalar_arrays_root_scalars_and_every_scalar_kind() {
        let value = serde_json::json!({
            "empty": [],
            "scalars": [null, true, 42, "text"],
            "nested": [[false]],
            "": "empty label"
        });
        let mut output = Vec::new();
        render_summary_value(&mut output, &value, 0).expect("summary");
        let output = String::from_utf8(output).expect("UTF-8");
        assert!(output.contains("none"));
        assert!(output.contains("- true"));
        assert!(output.contains("- 42"));
        assert!(output.contains("- text"));
        assert_eq!(human_label(""), "");
        assert_eq!(scalar(&serde_json::json!([])), "[]");
        assert_eq!(scalar(&serde_json::json!({})), "{}");
        assert_eq!(human_label("source_id"), "Source Id");

        let nested = serde_json::json!({"outer": {"inner": [{"leaf": "value"}]}});
        let mut nested_output = Vec::new();
        render_summary_value(&mut nested_output, &nested, 0).expect("nested summary");
        assert_eq!(
            String::from_utf8(nested_output).expect("nested UTF-8"),
            "Outer:\n  Inner:\n    -\n      Leaf: value\n"
        );

        let mut scalar_output = Vec::new();
        render_summary_value(&mut scalar_output, &serde_json::json!(false), 0)
            .expect("root scalar");
        assert_eq!(String::from_utf8(scalar_output).expect("UTF-8"), "false\n");

        for format in [
            OutputFormat::Json,
            OutputFormat::JsonPretty,
            OutputFormat::Summary,
        ] {
            let mut bytes = Vec::new();
            render_document(&mut bytes, &value, format).expect("render");
            assert!(!bytes.is_empty());
        }
    }
}
