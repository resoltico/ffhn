use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct MarkdownFence {
    pub(super) language: Option<String>,
    pub(super) body: String,
}

pub(super) fn marked_section(text: &str, marker_id: &str) -> Result<String, String> {
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

pub(super) fn production_source_text(text: &str) -> &str {
    text.split("\n#[cfg(test)]").next().unwrap_or(text)
}

pub(super) fn fenced_code_blocks(text: &str) -> Vec<MarkdownFence> {
    let mut fences = Vec::new();
    let mut current_language = None;
    let mut current_lines = Vec::new();
    let mut in_fence = false;

    for line in text.lines() {
        let trimmed = line.trim_start();
        if let Some(info_string) = trimmed.strip_prefix("```") {
            if in_fence {
                fences.push(MarkdownFence {
                    language: current_language.take(),
                    body: current_lines.join("\n"),
                });
                current_lines.clear();
                in_fence = false;
            } else {
                let language = info_string
                    .split_whitespace()
                    .next()
                    .filter(|language| !language.is_empty())
                    .map(str::to_owned);
                current_language = language;
                in_fence = true;
            }
            continue;
        }

        if in_fence {
            current_lines.push(line.to_owned());
        }
    }

    fences
}

pub(super) fn code_segments(text: &str) -> Vec<String> {
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

pub(super) fn markdown_link_targets(text: &str) -> Vec<String> {
    let mut targets = Vec::new();
    let mut remainder = text;

    while let Some(start) = remainder.find("](") {
        let after_start = &remainder[start + 2..];
        let Some(end) = after_start.find(')') else {
            break;
        };
        targets.push(
            after_start[..end]
                .trim()
                .trim_matches(['<', '>'])
                .to_owned(),
        );
        remainder = &after_start[end + 1..];
    }

    targets
}

pub(super) fn repo_file_mentions(text: &str) -> BTreeSet<String> {
    let mut mentions = BTreeSet::new();

    for segment in code_segments(text) {
        for raw_token in segment.split_whitespace() {
            let token = raw_token.trim_matches(|character: char| {
                matches!(
                    character,
                    '`' | '"' | '\'' | '(' | ')' | '[' | ']' | '{' | '}' | ',' | ';' | ':'
                )
            });
            if looks_like_repo_file_mention(token) {
                mentions.insert(token.to_owned());
            }
        }
    }

    mentions
}

pub(super) fn prose_lines_without_frontmatter_or_code(text: &str) -> Vec<(usize, String)> {
    let (line_offset, text) = strip_frontmatter_block(text);
    let mut lines = Vec::new();
    let mut in_fence = false;

    for (index, line) in text.lines().enumerate() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }

        let stripped = strip_inline_code_and_link_targets(line);
        if !stripped.trim().is_empty() {
            lines.push((line_offset + index + 1, stripped));
        }
    }

    lines
}

pub(super) fn looks_like_repo_file_mention(token: &str) -> bool {
    if token.is_empty()
        || token.starts_with("http://")
        || token.starts_with("https://")
        || token.starts_with("mailto:")
        || token.contains('*')
    {
        return false;
    }

    let normalized = token
        .trim_start_matches("./")
        .trim_start_matches("../")
        .trim_end_matches(['.', '/', ')']);

    if matches!(
        normalized,
        "AGENTS.md"
            | "Cargo.toml"
            | "Cargo.lock"
            | "README.md"
            | "CONTRIBUTING.md"
            | "changelog.md"
            | "check.sh"
            | "rust-toolchain.toml"
    ) {
        return true;
    }

    [
        ".codex/",
        ".devcontainer/",
        ".github/workflows/",
        "crates/",
        "docs/",
        "examples/",
        "fuzz/",
        "scripts/",
        "watchlist/",
        "xtask/",
    ]
    .iter()
    .any(|prefix| normalized.starts_with(prefix))
        && [
            ".html", ".json", ".lock", ".md", ".ps1", ".rs", ".sh", ".toml", ".yml",
        ]
        .iter()
        .any(|suffix| normalized.ends_with(suffix))
}

pub(super) fn resolve_repo_path(
    repo_root: &Path,
    markdown_file: &Path,
    mention: &str,
) -> Option<PathBuf> {
    let relative = mention.split('#').next().unwrap_or(mention);
    if relative.is_empty() {
        return Some(markdown_file.to_path_buf());
    }

    [
        markdown_file
            .parent()
            .expect("markdown parent")
            .join(relative),
        repo_root.join(relative),
    ]
    .into_iter()
    .find(|candidate| candidate.exists())
}

fn strip_frontmatter_block(text: &str) -> (usize, &str) {
    let (start_delimiter, end_delimiter, rest) = if let Some(rest) = text.strip_prefix("---\n") {
        ("---\n", "\n---\n", rest)
    } else if let Some(rest) = text.strip_prefix("---\r\n") {
        ("---\r\n", "\r\n---\r\n", rest)
    } else {
        return (0, text);
    };
    let Some(end) = rest.find(end_delimiter) else {
        return (0, text);
    };
    let body_start = end + end_delimiter.len();
    let line_offset = text[..(start_delimiter.len() + body_start)].lines().count();
    (line_offset, &rest[body_start..])
}

fn strip_inline_code_and_link_targets(line: &str) -> String {
    let mut output = String::new();
    let bytes = line.as_bytes();
    let mut index = 0;
    let mut in_inline_code = false;

    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'`' {
            in_inline_code = !in_inline_code;
            index += 1;
            continue;
        }
        if in_inline_code {
            index += 1;
            continue;
        }
        if byte == b'!' && bytes.get(index + 1) == Some(&b'[') {
            index += 2;
            while index < bytes.len() && bytes[index] != b']' {
                index += 1;
            }
            if index < bytes.len() && bytes.get(index + 1) == Some(&b'(') {
                index += 2;
                while index < bytes.len() && bytes[index] != b')' {
                    index += 1;
                }
                if index < bytes.len() {
                    index += 1;
                }
            } else if index < bytes.len() {
                index += 1;
            }
            continue;
        }

        if byte == b'[' {
            output.push('[');
            index += 1;
            while index < bytes.len() && bytes[index] != b']' {
                output.push(bytes[index] as char);
                index += 1;
            }
            if index < bytes.len() {
                output.push(']');
                if bytes.get(index + 1) == Some(&b'(') {
                    index += 2;
                    while index < bytes.len() && bytes[index] != b')' {
                        index += 1;
                    }
                    if index < bytes.len() {
                        index += 1;
                    }
                    continue;
                }
                index += 1;
            }
            continue;
        }

        output.push(byte as char);
        index += 1;
    }

    output
}

pub(super) fn repo_relative_path(repo_root: &Path, path: &Path) -> PathBuf {
    let relative = path
        .strip_prefix(repo_root)
        .expect("path inside repo root for repo-contract tests");
    let mut normalized = PathBuf::new();
    for component in relative.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            std::path::Component::Normal(part) => normalized.push(part),
            std::path::Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            std::path::Component::RootDir => {}
        }
    }
    normalized
}

pub(super) fn string_literals(text: &str) -> Vec<String> {
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

pub(super) fn extract_cli_operation_ids(text: &str) -> BTreeSet<String> {
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

pub(super) fn extract_document_ids(text: &str) -> BTreeSet<String> {
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

pub(super) fn looks_like_contract_document_id(candidate: &str) -> bool {
    let Some(suffix) = candidate.strip_prefix("ffhn.") else {
        return false;
    };
    matches!(suffix, "target" | "state" | "extraction_record") || suffix.ends_with("_report")
}

pub(super) fn assert_registered_operation_ids(
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

pub(super) fn assert_registered_document_ids(
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
