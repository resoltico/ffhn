//! Mutation-testing scope, command, evidence, and failure policy.

use std::{fs, path::Path};

use clap::ValueEnum;

use crate::{
    app::{check::ensure_cargo_subcommand, command::remove_dir_if_exists},
    hygiene::{HygieneCleanMode, clean_hygiene, ensure_hygiene, prepare_mutation_report_root},
    model::{CommandSpec, DynResult},
    tooling::{CargoQaToolSpec, rust_tooling},
};

use super::command::run_spec;

/// Selects the independently judged first-party mutation surface.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub(crate) enum MutantsScope {
    /// Mutate `ffhn-core` and `ffhn-cli`, judged by product tests.
    Runtime,
    /// Mutate `xtask`, judged by maintainer-tool tests.
    Tooling,
    /// Run the complete runtime campaign followed by the complete tooling campaign.
    #[default]
    All,
}

#[derive(Clone, Copy)]
struct ScopeSpec {
    name: &'static str,
    config: &'static str,
}

const RUNTIME: ScopeSpec = ScopeSpec {
    name: "runtime",
    config: ".cargo/mutants-runtime.toml",
};
const TOOLING: ScopeSpec = ScopeSpec {
    name: "tooling",
    config: ".cargo/mutants-tooling.toml",
};

/// Runs one selected scope or both complete mutation-testing campaigns.
pub(crate) fn run_mutants(
    repo_root: &Path,
    scope: MutantsScope,
    shard: Option<&str>,
    in_diff: Option<&Path>,
    iterate: bool,
) -> DynResult<()> {
    if scope == MutantsScope::All && shard.is_some() {
        return Err(
            "--shard requires an explicit --scope runtime or --scope tooling so its denominator has one meaning"
                .into(),
        );
    }
    if iterate && shard.is_some() {
        return Err(
            "--iterate is a local scope workflow and cannot be combined with --shard".into(),
        );
    }
    if iterate && in_diff.is_some() {
        return Err(
            "--iterate is a local scope workflow and cannot be combined with --in-diff".into(),
        );
    }

    let tooling = rust_tooling(repo_root)?;
    ensure_cargo_subcommand(
        CargoQaToolSpec {
            package_name: "cargo-mutants",
            subcommand_name: "mutants",
            expected_version: &tooling.cargo_mutants_version,
        },
        "Install the pinned mutation tool with `./scripts/bootstrap-rust-tools.sh install-mutation-tool`.",
    )?;
    clean_hygiene(repo_root, HygieneCleanMode::Safe)?;
    let report_root = prepare_mutation_report_root(repo_root)?;
    ensure_hygiene(repo_root)?;

    for selected in selected_scopes(scope) {
        let output_dir = report_root.join(selected.name);
        fs::create_dir_all(&output_dir)?;
        if !iterate {
            remove_dir_if_exists(&output_dir.join("mutants.out"))?;
            remove_dir_if_exists(&output_dir.join("mutants.out.old"))?;
        }
        let execution = run_spec(
            repo_root,
            &mutants_command(&output_dir, *selected, shard, in_diff, iterate),
        );
        if let Err(error) = execution {
            return Err(mutation_execution_error(error, selected.name));
        }
    }

    ensure_hygiene(repo_root)
}

fn selected_scopes(scope: MutantsScope) -> &'static [ScopeSpec] {
    match scope {
        MutantsScope::Runtime => std::slice::from_ref(&RUNTIME),
        MutantsScope::Tooling => std::slice::from_ref(&TOOLING),
        MutantsScope::All => &[RUNTIME, TOOLING],
    }
}

fn mutants_command(
    output_dir: &Path,
    scope: ScopeSpec,
    shard: Option<&str>,
    in_diff: Option<&Path>,
    iterate: bool,
) -> CommandSpec {
    let mut args = vec![
        "mutants".to_owned(),
        "--config".to_owned(),
        scope.config.to_owned(),
        "--output".to_owned(),
        output_dir.to_string_lossy().into_owned(),
    ];
    if let Some(shard) = shard {
        args.extend(["--shard".to_owned(), shard.to_owned()]);
    }
    if let Some(diff) = in_diff {
        args.extend(["--in-diff".to_owned(), diff.to_string_lossy().into_owned()]);
    }
    if iterate {
        args.push("--iterate".to_owned());
    }

    CommandSpec::new("cargo", args, false)
        .with_step_id(format!("mutants-{}", scope.name))
        .without_envs([
            "CARGO_TARGET_DIR",
            "CARGO_BUILD_BUILD_DIR",
            "CARGO_MUTANTS_MINIMUM_TEST_TIMEOUT",
        ])
}

fn mutation_execution_error(
    error: Box<dyn std::error::Error>,
    scope: &str,
) -> Box<dyn std::error::Error> {
    let message = error.to_string();
    let outcome = if message.contains("status exit status: 2") {
        "cargo-mutants found missed mutants; review `missed.txt` and the retained per-mutant logs"
    } else if message.contains("status exit status: 3") {
        "cargo-mutants timed out; review `timeout.txt` and the retained per-mutant logs before changing a timeout"
    } else if message.contains("status exit status: 4") {
        "cargo-mutants could not establish a passing unmutated baseline; repair the baseline before interpreting mutations"
    } else {
        return error;
    };
    format!("Mutation-testing scope `{scope}` failed: {outcome}.\n\n{message}").into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_selection_is_closed_and_all_is_dependency_ordered() {
        assert_eq!(selected_scopes(MutantsScope::Runtime)[0].name, "runtime");
        assert_eq!(selected_scopes(MutantsScope::Tooling)[0].name, "tooling");
        assert_eq!(
            selected_scopes(MutantsScope::All)
                .iter()
                .map(|scope| scope.name)
                .collect::<Vec<_>>(),
            ["runtime", "tooling"]
        );
    }

    #[test]
    fn command_always_uses_the_isolated_workspace_mode() {
        let safe = mutants_command(Path::new("/tmp/runtime"), RUNTIME, None, None, false);
        assert_eq!(
            safe.args,
            [
                "mutants",
                "--config",
                ".cargo/mutants-runtime.toml",
                "--output",
                "/tmp/runtime",
            ]
        );
        let ci = mutants_command(
            Path::new("/tmp/tooling"),
            TOOLING,
            Some("2/4"),
            Some(Path::new("changes.diff")),
            false,
        );
        let iterate = mutants_command(Path::new("/tmp/runtime"), RUNTIME, None, None, true);
        assert_eq!(iterate.args.last(), Some(&"--iterate".to_owned()));
        assert_eq!(
            iterate.removed_env,
            [
                "CARGO_BUILD_BUILD_DIR".to_owned(),
                "CARGO_MUTANTS_MINIMUM_TEST_TIMEOUT".to_owned(),
                "CARGO_TARGET_DIR".to_owned()
            ]
            .into_iter()
            .collect()
        );
        assert_eq!(
            ci.args,
            [
                "mutants",
                "--config",
                ".cargo/mutants-tooling.toml",
                "--output",
                "/tmp/tooling",
                "--shard",
                "2/4",
                "--in-diff",
                "changes.diff",
            ]
        );
    }

    #[test]
    fn cargo_mutants_exit_codes_keep_actionable_meaning() {
        for (status, expected) in [
            (2, "missed mutants"),
            (3, "timed out"),
            (4, "passing unmutated baseline"),
        ] {
            let error: Box<dyn std::error::Error> =
                format!("command failed with status exit status: {status}").into();
            assert!(
                mutation_execution_error(error, "runtime")
                    .to_string()
                    .contains(expected)
            );
        }
        let original: Box<dyn std::error::Error> = "other failure".into();
        assert_eq!(
            mutation_execution_error(original, "runtime").to_string(),
            "other failure"
        );
    }
}
