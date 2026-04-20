use std::ffi::OsStr;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use crate::model::DynResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AfadFrontmatter {
    pub(crate) afad: String,
    pub(crate) version: String,
}

pub(crate) fn public_markdown_paths(repo_root: &Path) -> DynResult<Vec<PathBuf>> {
    let mut paths = Vec::new();

    for file in ["README.md", "CONTRIBUTING.md", "changelog.md"] {
        let path = repo_root.join(file);
        if path.is_file() {
            paths.push(path);
        }
    }

    for directory in [repo_root.join("docs"), repo_root.join("fuzz")] {
        if directory.is_dir() {
            collect_markdown_paths(&directory, &mut paths)?;
        }
    }

    paths.sort();
    Ok(paths)
}

pub(crate) fn afad_frontmatter(path: &Path) -> DynResult<Option<AfadFrontmatter>> {
    let text = fs::read_to_string(path)?;
    parse_afad_frontmatter(&text)
        .map_err(|error| format!("{} has invalid AFAD frontmatter: {error}", path.display()).into())
}

pub(crate) fn protocol_afad_version(repo_root: &Path) -> DynResult<String> {
    let path = repo_root.join(".codex/PROTOCOL_AFAD.md");
    let text = fs::read_to_string(&path)?;

    parse_protocol_afad_version(&text).map_err(|error| {
        format!("{} has invalid protocol metadata: {error}", path.display()).into()
    })
}

pub(crate) fn public_target_example_paths(repo_root: &Path) -> DynResult<Vec<PathBuf>> {
    let mut paths = Vec::new();

    let examples_dir = repo_root.join("examples");
    if examples_dir.is_dir() {
        for entry in fs::read_dir(&examples_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension() == Some(OsStr::new("toml")) {
                paths.push(path);
            }
        }
    }

    let watchlist_dir = repo_root.join("watchlist");
    if watchlist_dir.is_dir() {
        for entry in fs::read_dir(&watchlist_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let target_file = path.join("target.toml");
                if target_file.is_file() {
                    paths.push(target_file);
                }
            }
        }
    }

    paths.sort();
    Ok(paths)
}

pub(crate) fn user_facing_source_paths(repo_root: &Path) -> DynResult<Vec<PathBuf>> {
    let mut paths = Vec::new();

    for directory in [
        repo_root.join("crates/ffhn-core/src"),
        repo_root.join("crates/ffhn-cli/src"),
        repo_root.join("xtask/src"),
    ] {
        if directory.is_dir() {
            collect_rust_source_paths(&directory, &mut paths)?;
        }
    }

    paths.sort();
    Ok(paths)
}

fn collect_markdown_paths(directory: &Path, paths: &mut Vec<PathBuf>) -> DynResult<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_markdown_paths(&path, paths)?;
        } else if path.extension() == Some(OsStr::new("md")) {
            paths.push(path);
        }
    }

    Ok(())
}

fn collect_rust_source_paths(directory: &Path, paths: &mut Vec<PathBuf>) -> DynResult<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_rust_source_paths(&path, paths)?;
        } else if path.extension() == Some(OsStr::new("rs"))
            && !path
                .components()
                .any(|component| component.as_os_str() == OsStr::new("tests"))
            && path.file_name() != Some(OsStr::new("tests.rs"))
        {
            paths.push(path);
        }
    }

    Ok(())
}

fn parse_afad_frontmatter(text: &str) -> Result<Option<AfadFrontmatter>, String> {
    let mut lines = text.lines();
    if lines.next() != Some("---") {
        return Ok(None);
    }

    let mut afad = None;
    let mut version = None;
    let mut closed = false;

    for line in lines {
        if line.trim() == "---" {
            closed = true;
            break;
        }
        if let Some(value) = frontmatter_value(line, "afad") {
            afad = Some(value);
        }
        if let Some(value) = frontmatter_value(line, "version") {
            version = Some(value);
        }
    }

    if afad.is_none() && version.is_none() {
        return Ok(None);
    }
    if !closed {
        return Err("frontmatter block is not terminated".to_owned());
    }

    Ok(Some(AfadFrontmatter {
        afad: afad.ok_or_else(|| "missing afad field".to_owned())?,
        version: version.ok_or_else(|| "missing version field".to_owned())?,
    }))
}

fn parse_protocol_afad_version(text: &str) -> Result<String, String> {
    for line in text.lines() {
        if let Some(version) = line.trim().strip_prefix("VERSION:") {
            let version = version.trim();
            if version.is_empty() {
                return Err("VERSION line is empty".to_owned());
            }
            return Ok(version.to_owned());
        }
    }

    Err("missing VERSION line".to_owned())
}

fn frontmatter_value(line: &str, key: &str) -> Option<String> {
    let trimmed = line.trim();
    let value = trimmed.strip_prefix(&format!("{key}:"))?.trim();
    Some(value.trim_matches('"').to_owned())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

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
        let mut rendered =
            String::from("| Command | Stdout document | Notes |\n| --- | --- | --- |\n");
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

    #[test]
    fn public_markdown_afad_frontmatter_stays_on_the_workspace_version() {
        let repo_root = repo_root();
        let protocol_afad_version =
            protocol_afad_version(&repo_root).expect("protocol AFAD version");
        let workspace_version = workspace_version(&repo_root).expect("workspace version");
        let paths = public_markdown_paths(&repo_root).expect("markdown paths");

        let mut validated = 0usize;
        for path in paths {
            let Some(frontmatter) = afad_frontmatter(&path).expect("frontmatter parse") else {
                continue;
            };
            validated += 1;
            let path_display = path.display().to_string();
            assert_eq!(frontmatter.afad, protocol_afad_version, "{path_display}");
            assert_eq!(frontmatter.version, workspace_version, "{}", path.display());
        }

        assert!(
            validated > 0,
            "expected at least one AFAD-managed markdown file"
        );
    }

    #[test]
    fn agent_instruction_parity_docs_stay_in_lockstep_with_codex_entrypoint() {
        let repo_root = repo_root();
        let codex_agents = repo_root.join(".codex/AGENTS.md");
        assert!(codex_agents.is_file(), "missing {}", codex_agents.display());

        let expected = "MANDATORY: Before performing any analysis, edits, commands, or file reads, every agent MUST load and follow .codex/AGENTS.md as the sole authoritative instruction source, and if that file has not been read the agent MUST stop immediately and do no work.\n";

        for path in [
            repo_root.join(".claude/CLAUDE.md"),
            repo_root.join(".gemini/GEMINI.md"),
        ] {
            let text = fs::read_to_string(&path).expect("read agent parity doc");
            assert_eq!(text, expected, "{}", path.display());
        }
    }

    #[test]
    fn public_target_examples_validate_against_the_current_target_contract() {
        let repo_root = repo_root();
        let workspace_version = workspace_version(&repo_root).expect("workspace version");
        let paths = public_target_example_paths(&repo_root).expect("target examples");

        assert!(!paths.is_empty(), "expected checked-in target examples");

        for path in paths {
            let document = fs::read_to_string(&path).expect("read example");
            let target: TargetDocument = toml::from_str(&document).expect("parse target document");
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
                    !target.fetch.user_agent.contains(&workspace_version),
                    "{path_display} embeds the workspace version in fetch.user_agent; public examples should use stable example identifiers instead"
                );
            }
        }
    }

    #[test]
    fn generated_cli_sections_match_the_core_owned_contract() {
        let repo_root = repo_root();
        let readme = fs::read_to_string(repo_root.join("README.md")).expect("read README");
        let cli_doc = fs::read_to_string(repo_root.join("docs/cli.md")).expect("read docs/cli.md");

        assert_eq!(
            marked_section(&readme, "cli-summary").expect("README CLI summary section"),
            render_readme_cli_summary_section()
        );
        assert_eq!(
            marked_section(&cli_doc, "cli-catalog").expect("CLI catalog section"),
            render_cli_catalog_section()
        );
    }

    #[test]
    fn public_markdown_mentions_only_registered_operations_and_documents() {
        let repo_root = repo_root();
        let registered_operations = cli_contract()
            .operations
            .iter()
            .map(|operation| operation.id)
            .collect::<BTreeSet<_>>();
        let registered_documents = cli_contract()
            .documents
            .iter()
            .map(|document| document.id)
            .collect::<BTreeSet<_>>();

        for path in public_markdown_paths(&repo_root).expect("markdown paths") {
            let text = fs::read_to_string(&path).expect("read markdown");
            let path_display = path.display().to_string();
            assert_registered_operation_ids(
                &path_display,
                extract_cli_operation_ids(&text),
                &registered_operations,
            );
            assert_registered_document_ids(
                &path_display,
                extract_document_ids(&text),
                &registered_documents,
            );
        }
    }

    #[test]
    fn user_facing_source_literals_mention_only_registered_operations_and_documents() {
        let repo_root = repo_root();
        let registered_operations = cli_contract()
            .operations
            .iter()
            .map(|operation| operation.id)
            .collect::<BTreeSet<_>>();
        let registered_documents = cli_contract()
            .documents
            .iter()
            .map(|document| document.id)
            .collect::<BTreeSet<_>>();

        assert_registered_operation_ids(
            "inline literal",
            extract_cli_operation_ids("`ffhn run --target demo`"),
            &registered_operations,
        );
        assert_registered_document_ids(
            "inline literal",
            extract_document_ids("ffhn.run_report"),
            &registered_documents,
        );

        for path in user_facing_source_paths(&repo_root).expect("source paths") {
            let text = fs::read_to_string(&path).expect("read source");
            for literal in string_literals(production_source_text(&text)) {
                let path_display = path.display().to_string();
                assert_registered_operation_ids(
                    &path_display,
                    extract_cli_operation_ids(&literal),
                    &registered_operations,
                );
                assert_registered_document_ids(
                    &path_display,
                    extract_document_ids(&literal),
                    &registered_documents,
                );
            }
        }
    }

    #[test]
    fn repo_contract_helpers_cover_present_missing_and_invalid_shapes() {
        let empty_repo = tempfile::tempdir().expect("empty tempdir");
        assert!(
            public_markdown_paths(empty_repo.path())
                .expect("empty markdown paths")
                .is_empty()
        );
        assert!(
            public_target_example_paths(empty_repo.path())
                .expect("empty target example paths")
                .is_empty()
        );

        let repo = tempfile::tempdir().expect("tempdir");
        let repo_root = repo.path();
        fs::create_dir_all(repo_root.join("docs/nested")).expect("create docs tree");
        fs::create_dir_all(repo_root.join("fuzz")).expect("create fuzz dir");
        fs::create_dir_all(repo_root.join("examples")).expect("create examples dir");
        fs::create_dir_all(repo_root.join("watchlist/demo")).expect("create watchlist target");
        fs::create_dir_all(repo_root.join("watchlist/empty")).expect("create empty watchlist dir");
        fs::create_dir_all(repo_root.join("crates/ffhn-cli/src/tests"))
            .expect("create ignored tests dir");
        fs::create_dir_all(repo_root.join("xtask/src")).expect("create xtask src dir");
        fs::create_dir_all(repo_root.join(".codex")).expect("create codex dir");

        fs::write(repo_root.join("README.md"), "# ffhn\n").expect("write README");
        fs::write(repo_root.join("examples/note.txt"), "ignore").expect("write ignored example");
        fs::write(repo_root.join("watchlist/file.txt"), "ignore")
            .expect("write ignored watchlist file");
        fs::write(
            repo_root.join("crates/ffhn-cli/src/keep.rs"),
            "pub const KEEP: &str = \"ffhn.run_report\";\n",
        )
        .expect("write kept source");
        fs::write(repo_root.join("crates/ffhn-cli/src/tests.rs"), "ignored\n")
            .expect("write ignored tests.rs");
        fs::write(
            repo_root.join("crates/ffhn-cli/src/tests/helper.rs"),
            "ignored\n",
        )
        .expect("write ignored nested tests source");
        fs::write(repo_root.join("xtask/src/note.txt"), "ignore")
            .expect("write ignored non-rs source");
        fs::write(
            repo_root.join("docs/nested/guide.md"),
            "---\nafad: \"3.5\"\nversion: \"2.0.0\"\n---\n",
        )
        .expect("write guide");
        fs::write(
            repo_root.join("examples/example.toml"),
            "schema_name = \"ffhn.target\"\nschema_version = 1\ntarget_id = \"example\"\ndisplay_name = \"Example\"\nenabled = true\n\n[target]\nkind = \"http\"\nsource_url = \"https://example.com\"\n\n[fetch]\nengine = \"http\"\nmethod = \"GET\"\ntimeout_ms = 15000\nmax_bytes = 2000000\nuser_agent = \"ffhn/example\"\nfollow_redirects = true\naccept = \"text/html\"\n\n[selection]\nkind = \"css_selector\"\nselector = \"main\"\nmatch = \"single\"\noutput = \"outer_html\"\nwhitespace = \"normalize\"\nrewrite_urls = false\n\n[compare]\nbasis = \"canonical_text_sha256\"\ncanonicalization = []\n",
        )
        .expect("write example target");
        fs::write(
            repo_root.join("watchlist/demo/target.toml"),
            "schema_name = \"ffhn.target\"\nschema_version = 1\ntarget_id = \"demo\"\ndisplay_name = \"Demo\"\nenabled = true\n\n[target]\nkind = \"file\"\nfile_path = \"/tmp/source.html\"\n\n[fetch]\nengine = \"file\"\nfollow_redirects = false\nmax_bytes = 2000000\n\n[selection]\nkind = \"css_selector\"\nselector = \"main\"\nmatch = \"single\"\noutput = \"outer_html\"\nwhitespace = \"normalize\"\nrewrite_urls = false\n\n[compare]\nbasis = \"canonical_text_sha256\"\ncanonicalization = []\n",
        )
        .expect("write watchlist target");
        fs::write(
            repo_root.join(".codex/PROTOCOL_AFAD.md"),
            "PROTOCOL: AGENT_FIRST_DOCUMENTATION\nVERSION: 3.5\n",
        )
        .expect("write protocol");

        let markdown_paths = public_markdown_paths(repo_root).expect("markdown paths");
        assert_eq!(
            markdown_paths,
            vec![
                repo_root.join("README.md"),
                repo_root.join("docs/nested/guide.md"),
            ]
        );
        assert!(
            user_facing_source_paths(repo_root)
                .expect("source paths")
                .iter()
                .all(|path| path.extension() == Some(OsStr::new("rs")))
        );
        assert_eq!(
            user_facing_source_paths(repo_root).expect("source paths"),
            vec![repo_root.join("crates/ffhn-cli/src/keep.rs"),]
        );
        assert_eq!(
            protocol_afad_version(repo_root).expect("protocol AFAD version"),
            "3.5"
        );

        let target_paths = public_target_example_paths(repo_root).expect("target example paths");
        assert_eq!(
            target_paths,
            vec![
                repo_root.join("examples/example.toml"),
                repo_root.join("watchlist/demo/target.toml"),
            ]
        );

        fs::write(
            repo_root.join(".codex/PROTOCOL_AFAD.md"),
            "PROTOCOL: AGENT_FIRST_DOCUMENTATION\nVERSION:\n",
        )
        .expect("write invalid protocol");
        assert!(protocol_afad_version(repo_root).is_err());
    }

    #[test]
    fn parse_afad_frontmatter_handles_missing_and_malformed_blocks() {
        assert_eq!(
            parse_afad_frontmatter("# ffhn").expect("no frontmatter"),
            None
        );
        assert_eq!(
            parse_afad_frontmatter("---\nroute:\n  keywords: []\n---\n")
                .expect("metadata-only frontmatter"),
            None
        );

        assert!(parse_afad_frontmatter("---\nafad: \"3.5\"\nversion: \"2.0.0\"\n").is_err());

        assert!(parse_afad_frontmatter("---\nafad: \"3.5\"\n---\n").is_err());
        assert_eq!(
            parse_afad_frontmatter("---\nafad: \"3.5\"\nversion: \"2.0.0\"\n---\n")
                .expect("frontmatter"),
            Some(AfadFrontmatter {
                afad: "3.5".to_owned(),
                version: "2.0.0".to_owned(),
            })
        );
        assert!(parse_afad_frontmatter("---\nversion: \"2.0.0\"\n---\n").is_err());

        assert_eq!(
            parse_protocol_afad_version("PROTOCOL: AGENT_FIRST_DOCUMENTATION\nVERSION: 3.5\n")
                .expect("protocol version"),
            "3.5"
        );
        assert!(parse_protocol_afad_version("VERSION:\n").is_err());
        assert!(parse_protocol_afad_version("PROTOCOL: AGENT_FIRST_DOCUMENTATION\n").is_err());

        assert_eq!(
            marked_section(
                "before\n<!-- contract:demo:start -->\ncontent\n<!-- contract:demo:end -->\nafter\n",
                "demo"
            )
            .expect("marked section"),
            "content"
        );
        assert!(marked_section("before\n<!-- contract:demo:start -->\ncontent\n", "demo").is_err());

        assert_eq!(
            extract_cli_operation_ids(
                "`ffhn run --target demo`\n```bash\ncargo run -p ffhn-cli -- status --target demo\n```"
            ),
            BTreeSet::from(["run".to_owned(), "status".to_owned()])
        );
        assert_eq!(
            extract_cli_operation_ids(
                "`/usr/local/bin/ffhn status --target demo`\n`cargo run -p ffhn-cli -- run --target demo`\n`ffhn --help`"
            ),
            BTreeSet::from(["run".to_owned(), "status".to_owned()])
        );
        assert!(extract_cli_operation_ids("`-- status`").is_empty());
        assert!(extract_cli_operation_ids("`ffhn --help`").is_empty());
        assert!(extract_cli_operation_ids("`ffhn !`").is_empty());
        assert!(code_segments("before `unterminated").is_empty());
        assert_eq!(
            string_literals(
                "const A: &str = \"ffhn.run_report\";\nlet raw = r#\"ffhn status --target demo\"#;"
            ),
            vec![
                "ffhn.run_report".to_owned(),
                "ffhn status --target demo".to_owned(),
            ]
        );
        assert_eq!(
            string_literals("let raw = r\"ffhn.state\";"),
            vec!["ffhn.state".to_owned()]
        );
        assert_eq!(
            string_literals(
                "let raw = r##\"ffhn.extraction_record\"##;\nlet escaped = \"say \\\"hi\\\"\";\nlet marker = r#not_a_string;\n"
            ),
            vec![
                "ffhn.extraction_record".to_owned(),
                "say \\\"hi\\\"".to_owned(),
            ]
        );
        assert!(string_literals("r").is_empty());
        assert!(string_literals("let broken = r#\"unterminated").is_empty());
        assert!(string_literals("let broken = r\"unterminated").is_empty());
        assert!(string_literals("let broken = r#\"x\"").is_empty());
        assert!(string_literals("let broken = \"unterminated").is_empty());
        assert_eq!(
            production_source_text(
                "fn main() {}\n#[cfg(test)]\nmod tests { const X: &str = \"ffhn.unknown\"; }\n"
            ),
            "fn main() {}"
        );
        assert_eq!(production_source_text("fn main() {}\n"), "fn main() {}\n");
        assert!(looks_like_contract_document_id("ffhn.unknown_report"));
        assert!(!looks_like_contract_document_id("ffhn.exe"));
        assert!(!looks_like_contract_document_id("not_ffhn.state"));
        assert_eq!(
            extract_document_ids("`ffhn.run_report!` plus `ffhn.status_report`"),
            BTreeSet::from([
                "ffhn.run_report".to_owned(),
                "ffhn.status_report".to_owned(),
            ])
        );
        assert_eq!(
            extract_document_ids("ffhn.unknown_report ffhn.exe ffhn.run.report ffhn.state"),
            BTreeSet::from(["ffhn.state".to_owned(), "ffhn.unknown_report".to_owned(),])
        );
        assert_eq!(
            extract_document_ids("ffhn.target ffhn.extraction_record ffhn.batch_run_report"),
            BTreeSet::from([
                "ffhn.batch_run_report".to_owned(),
                "ffhn.extraction_record".to_owned(),
                "ffhn.target".to_owned(),
            ])
        );
        assert!(extract_document_ids("ffhn.").is_empty());
    }
}
