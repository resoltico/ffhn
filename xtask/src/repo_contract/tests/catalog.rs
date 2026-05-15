use std::ffi::OsStr;
#[cfg(unix)]
use std::fmt::Write as _;
use std::path::Path;

use ffhn_core::TargetDocument;
#[cfg(unix)]
use ffhn_core::cli_contract;

#[cfg(unix)]
pub(super) fn render_cli_catalog_section() -> String {
    let mut rendered =
        String::from("| Command | Structured stdout document | Notes |\n| --- | --- | --- |\n");
    for (usage, output_document_id, summary) in command_catalog_rows() {
        let _ = writeln!(
            rendered,
            "| `{usage}` | `{output_document_id}` | {summary} |"
        );
    }
    rendered.push_str("\nPer-operation help contract:\n\n");
    for operation in cli_contract().operations {
        let _ = writeln!(rendered, "### `{}`\n", operation.id);
        let _ = writeln!(rendered, "Summary: {}\n", operation.help_summary);
        let _ = writeln!(rendered, "Usage: `{}`\n", operation.usage);

        rendered.push_str("Options:\n\n");
        for (index, argument) in operation.arguments.iter().enumerate() {
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

        rendered.push_str("\nExamples:\n\n");
        for (index, example) in operation.examples.iter().enumerate() {
            let _ = writeln!(rendered, "{}. `{example}`", index + 1);
        }

        rendered.push_str("\nOutput:\n\n");
        for (index, note) in operation.output_notes.iter().enumerate() {
            let _ = writeln!(rendered, "{}. {}", index + 1, note);
        }

        rendered.push_str("\nOperational notes:\n\n");
        for (index, note) in operation.operational_notes.iter().enumerate() {
            let _ = writeln!(rendered, "{}. {}", index + 1, note);
        }
        rendered.push('\n');
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

pub(super) fn assert_public_target_example_contract(
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
        assert_eq!(target.target_id(), directory_name, "{}", path.display());
    }

    if target.source_url().is_some() {
        let path_display = path.display().to_string();
        assert!(
            !target
                .fetch_user_agent()
                .expect("http examples must expose fetch.user_agent")
                .contains(workspace_version),
            "{path_display} embeds the workspace version in fetch.user_agent; public examples should use stable example identifiers instead"
        );
    }
}

#[cfg(unix)]
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
