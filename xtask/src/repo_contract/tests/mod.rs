use std::collections::BTreeSet;
use std::process::Command;

use super::*;
use crate::plan::workspace_version;
use ffhn_core::{CLI_OPERATION_RUN_ID, TargetDocument, cli_contract, cli_operation};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

fn command_catalog_rows() -> Vec<(String, String, String)> {
    cli_contract()
        .operations
        .iter()
        .flat_map(|operation| {
            operation.invocations.iter().map(|invocation| {
                (
                    invocation.usage.to_owned(),
                    invocation.output_document_id.to_owned(),
                    invocation.analysis_summary.to_owned(),
                )
            })
        })
        .collect()
}

fn render_readme_cli_summary_section() -> String {
    let mut rendered = String::from("| Command | Stdout document | Notes |\n| --- | --- | --- |\n");
    for (usage, output_document_id, summary) in command_catalog_rows() {
        let _ = writeln!(
            rendered,
            "| `{usage}` | `{output_document_id}` | {summary} |"
        );
    }
    rendered.trim_end().to_owned()
}

fn render_cli_catalog_section() -> String {
    let run = cli_operation(CLI_OPERATION_RUN_ID).expect("run operation");

    let mut rendered =
        String::from("| Command | Structured stdout document | Notes |\n| --- | --- | --- |\n");
    for (usage, output_document_id, summary) in command_catalog_rows() {
        let _ = writeln!(
            rendered,
            "| `{usage}` | `{output_document_id}` | {summary} |"
        );
    }
    rendered.push_str("\nThe maintained help text is:\n\n");
    for (index, operation) in cli_contract().operations.iter().enumerate() {
        let _ = writeln!(
            rendered,
            "{}. `{}`: {}",
            index + 1,
            operation.id,
            operation.help_summary
        );
    }

    rendered.push_str("\n`run` supports:\n\n");
    for (index, argument) in run.arguments.iter().enumerate() {
        let label = if let Some(value_name) = argument.value_name {
            format!("--{} <{}>", argument.long_name, value_name)
        } else {
            format!("--{}", argument.long_name)
        };
        let default = argument
            .default_value
            .map(|value| format!(" Default: `{value}`."))
            .unwrap_or_default();
        let _ = writeln!(
            rendered,
            "{}. `{label}`: {}{}",
            index + 1,
            argument.help_summary,
            default
        );
    }

    rendered.push_str("\nExecution modes:\n\n");
    for (index, mode) in cli_contract().execution_modes.iter().enumerate() {
        let _ = writeln!(rendered, "{}. `{}`: {}", index + 1, mode.id, mode.summary);
    }

    rendered.push_str("\nHard limitations:\n\n");
    for (index, limit) in cli_contract().hard_limits.iter().enumerate() {
        let _ = writeln!(rendered, "{}. {}", index + 1, limit.summary);
    }

    rendered.trim_end().to_owned()
}

fn assert_public_target_example_contract(
    path: &Path,
    target: &TargetDocument,
    workspace_version: &str,
) {
    target.validate().expect("validate target document");

    if path.file_name() == Some(OsStr::new("target.toml")) {
        let directory_name = path
            .parent()
            .and_then(Path::file_name)
            .and_then(OsStr::to_str)
            .expect("target directory name");
        assert_eq!(target.target_id, directory_name, "{}", path.display());
    }

    if target.target.source_url.is_some() {
        let path_display = path.display().to_string();
        assert!(
            !target.fetch.user_agent.contains(workspace_version),
            "{path_display} embeds the workspace version in fetch.user_agent; public examples should use stable example identifiers instead"
        );
    }
}

fn marked_section(text: &str, marker_id: &str) -> Result<String, String> {
    let start_marker = format!("<!-- contract:{marker_id}:start -->");
    let end_marker = format!("<!-- contract:{marker_id}:end -->");
    let start = text
        .find(&start_marker)
        .ok_or_else(|| format!("missing start marker {start_marker}"))?;
    let body_start = start + start_marker.len();
    let end = text[body_start..]
        .find(&end_marker)
        .ok_or_else(|| format!("missing end marker {end_marker}"))?
        + body_start;
    Ok(text[body_start..end].trim().to_owned())
}

fn production_source_text(text: &str) -> &str {
    text.split("\n#[cfg(test)]").next().unwrap_or(text)
}

fn code_segments(text: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut in_fence = false;

    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }

        if in_fence {
            segments.push(line.to_owned());
            continue;
        }

        let mut remainder = line;
        while let Some(start) = remainder.find('`') {
            let after_start = &remainder[start + 1..];
            let Some(end) = after_start.find('`') else {
                break;
            };
            segments.push(after_start[..end].to_owned());
            remainder = &after_start[end + 1..];
        }
    }

    segments
}

fn string_literals(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut literals = Vec::new();
    let mut index = 0usize;

    while index < bytes.len() {
        if bytes[index] == b'r' {
            let mut hashes = 0usize;
            while index + 1 + hashes < bytes.len() && bytes[index + 1 + hashes] == b'#' {
                hashes += 1;
            }
            let quote_index = index + 1 + hashes;
            if quote_index < bytes.len() && bytes[quote_index] == b'"' {
                let content_start = quote_index + 1;
                let mut cursor = content_start;
                while cursor < bytes.len() {
                    let closes_raw = bytes[cursor] == b'"'
                        && cursor + hashes < bytes.len()
                        && (hashes == 0
                            || bytes[cursor + 1..=cursor + hashes]
                                .iter()
                                .all(|byte| *byte == b'#'));
                    if closes_raw {
                        literals.push(text[content_start..cursor].to_owned());
                        index = cursor + hashes + 1;
                        break;
                    }
                    cursor += 1;
                }
                if cursor >= bytes.len() {
                    break;
                }
                continue;
            }
        }

        if bytes[index] == b'"' {
            let content_start = index + 1;
            let mut cursor = content_start;
            while cursor < bytes.len() {
                if bytes[cursor] == b'"' && bytes[cursor.saturating_sub(1)] != b'\\' {
                    literals.push(text[content_start..cursor].to_owned());
                    index = cursor + 1;
                    break;
                }
                cursor += 1;
            }
            if cursor >= bytes.len() {
                break;
            }
            continue;
        }

        index += 1;
    }

    literals
}

fn extract_cli_operation_ids(text: &str) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();

    for segment in code_segments(text) {
        let tokens = segment.split_whitespace().collect::<Vec<_>>();
        for (index, token) in tokens.iter().enumerate() {
            let launch_token =
                *token == "ffhn" || token.rsplit('/').next().is_some_and(|last| last == "ffhn");
            let cargo_run_separator = *token == "--"
                && index > 0
                && tokens[index - 1]
                    .rsplit('/')
                    .next()
                    .is_some_and(|last| last == "ffhn-cli");
            let candidate = if launch_token || cargo_run_separator {
                tokens.get(index + 1).copied()
            } else {
                None
            };
            let Some(candidate) = candidate else {
                continue;
            };

            let candidate = candidate.trim_matches(|character: char| {
                !character.is_ascii_alphanumeric() && character != '-'
            });
            if !candidate.is_empty() && !candidate.starts_with('-') {
                ids.insert(candidate.to_owned());
            }
        }
    }

    ids
}

fn extract_document_ids(text: &str) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    let bytes = text.as_bytes();
    let mut index = 0usize;

    while index + 5 <= bytes.len() {
        if bytes[index..].starts_with(b"ffhn.") {
            let mut end = index + 5;
            while end < bytes.len() {
                let character = bytes[end] as char;
                if character.is_ascii_lowercase() || character == '_' || character == '.' {
                    end += 1;
                } else {
                    break;
                }
            }
            if end > index + 5 {
                let candidate = std::str::from_utf8(&bytes[index..end])
                    .expect("document ids are ASCII")
                    .to_owned();
                if looks_like_contract_document_id(&candidate) {
                    ids.insert(candidate);
                }
            }
            index = end;
        } else {
            index += 1;
        }
    }

    ids
}

fn looks_like_contract_document_id(candidate: &str) -> bool {
    let Some(suffix) = candidate.strip_prefix("ffhn.") else {
        return false;
    };
    matches!(suffix, "target" | "state" | "extraction_record") || suffix.ends_with("_report")
}

fn assert_registered_operation_ids(
    path_display: &str,
    ids: BTreeSet<String>,
    registered_operations: &BTreeSet<&str>,
) {
    for operation_id in ids {
        assert!(
            registered_operations.contains(operation_id.as_str()),
            "{path_display} mentions unknown FFHN operation id `{operation_id}`"
        );
    }
}

fn assert_registered_document_ids(
    path_display: &str,
    ids: BTreeSet<String>,
    registered_documents: &BTreeSet<&str>,
) {
    for document_id in ids {
        assert!(
            registered_documents.contains(document_id.as_str()),
            "{path_display} mentions unknown FFHN document id `{document_id}`"
        );
    }
}

fn bash_stdout_lines(repo_root: &Path, command: &str) -> Vec<String> {
    let output = Command::new("bash")
        .current_dir(repo_root)
        .arg("-lc")
        .arg(command)
        .output()
        .expect("run bash helper");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "bash helper failed: {command}\n{stderr}"
    );
    String::from_utf8(output.stdout)
        .expect("utf-8 stdout")
        .lines()
        .map(str::to_owned)
        .collect()
}

mod docs;
mod examples;
mod parsing;
