use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::io::Write;
use std::path::Path;

use clap::error::ErrorKind;
use ffhn_core::{
    BatchRunEntry, BatchRunReport, BatchRunReportInput, CoreError, ProcessErrorDetail, RunMode,
    RunOutcome, TargetId, TargetPaths, batch_run_report_write_error,
    duplicate_target_ids_usage_error, run_batch, run_once, run_once_dry_run,
    run_report_write_error, status, status_report_write_error, validate_target,
};

use crate::args::{Command, RunCommand, parse_cli};
use crate::error::{CLI_OUTPUT_WRITE_ERROR, write_cli_error};
use crate::help::try_handle_top_level_request;
use crate::render::render_json_document;
use crate::{EXIT_CODE_FATAL, EXIT_CODE_RUN_FAILED, EXIT_CODE_USAGE};

/// Entry point for the FFHN CLI.
pub fn run(
    args: impl IntoIterator<Item = String>,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> i32 {
    let raw_args: Vec<String> = args.into_iter().collect();
    match try_handle_top_level_request(&raw_args, stdout) {
        Ok(true) => return 0,
        Err(_) => return report_cli_output_error(stderr),
        Ok(false) => {}
    }

    let cli = match parse_cli(raw_args) {
        Ok(cli) => cli,
        Err(error) => {
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) {
                return if write!(stdout, "{error}").is_ok() {
                    0
                } else {
                    report_cli_output_error(stderr)
                };
            }
            let _ = write_cli_error(stderr, &error.to_string());
            return EXIT_CODE_USAGE;
        }
    };

    match cli.command {
        Command::Run(command) => {
            if let Some(duplicate) = duplicate_target_id(&command) {
                let _ = write_cli_error(stderr, &duplicate_target_ids_usage_error(duplicate));
                return EXIT_CODE_USAGE;
            }

            let selection = match selected_targets(&command) {
                Ok(targets) => targets,
                Err(error) => {
                    let _ = write_cli_error(stderr, &error.to_string());
                    return EXIT_CODE_FATAL;
                }
            };

            if let SelectedTargets::Explicit(targets) = &selection
                && targets.len() == 1
            {
                let paths = TargetPaths::new(command.watch_root, targets[0].clone());
                let run_result = if command.dry_run {
                    run_once_dry_run(&paths)
                } else {
                    run_once(&paths)
                };
                return match run_result {
                    Ok(report) => {
                        if render_json_document(stdout, &report).is_err() {
                            let _ = write_cli_error(stderr, &run_report_write_error());
                            EXIT_CODE_FATAL
                        } else if run_report_requires_failed_exit(&report) {
                            EXIT_CODE_RUN_FAILED
                        } else {
                            0
                        }
                    }
                    Err(error) => {
                        let _ = write_cli_error(stderr, &error.to_string());
                        EXIT_CODE_FATAL
                    }
                };
            }

            let batch_result = match selection {
                SelectedTargets::Explicit(targets) => run_batch(
                    &command.watch_root,
                    &targets,
                    requested_run_mode(command.dry_run),
                    command.jobs,
                ),
                SelectedTargets::Discovered(targets) => run_discovered_batch(
                    &command.watch_root,
                    targets,
                    requested_run_mode(command.dry_run),
                    command.jobs,
                ),
            };

            render_batch_result(batch_result, stdout, stderr)
        }
        Command::Status(command) => {
            let paths = TargetPaths::new(command.watch_root, command.target);
            match status(&paths) {
                Ok(report) => {
                    if render_json_document(stdout, &report).is_err() {
                        let _ = write_cli_error(stderr, &status_report_write_error());
                        return EXIT_CODE_FATAL;
                    }
                    0
                }
                Err(error) => {
                    let _ = write_cli_error(stderr, &error.to_string());
                    EXIT_CODE_FATAL
                }
            }
        }
    }
}

fn report_cli_output_error(stderr: &mut impl Write) -> i32 {
    let _ = write_cli_error(stderr, CLI_OUTPUT_WRITE_ERROR);
    EXIT_CODE_FATAL
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum SelectedTargets {
    Explicit(Vec<TargetId>),
    Discovered(Vec<DiscoveredTarget>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DiscoveredTarget {
    pub(crate) requested_id: String,
    pub(crate) validated_id: Option<TargetId>,
    pub(crate) validation_message: Option<String>,
}

fn selected_targets(command: &RunCommand) -> Result<SelectedTargets, CoreError> {
    if !command.all {
        return Ok(SelectedTargets::Explicit(command.targets.clone()));
    }

    Ok(SelectedTargets::Discovered(discover_watch_root_targets(
        &command.watch_root,
    )?))
}

fn duplicate_target_id(command: &RunCommand) -> Option<&str> {
    if command.all {
        return None;
    }

    let mut seen = BTreeSet::new();
    command
        .targets
        .iter()
        .find_map(|target_id| (!seen.insert(target_id.as_str())).then_some(target_id.as_str()))
}

pub(crate) fn discover_watch_root_targets(
    watch_root: &Path,
) -> Result<Vec<DiscoveredTarget>, CoreError> {
    let mut targets = Vec::new();
    if !watch_root.exists() {
        return Err(CoreError::io(
            watch_root,
            io::Error::new(io::ErrorKind::NotFound, "watch root does not exist"),
        ));
    }
    if !watch_root.is_dir() {
        return Err(CoreError::io(
            watch_root,
            io::Error::other("watch root is not a directory"),
        ));
    }

    let read_dir = fs::read_dir(watch_root)
        .map_err(|error| CoreError::io(watch_root, error))?
        .map(|entry| entry.map(|entry| entry.path()));
    let mut entries = collect_watch_root_directories(watch_root, read_dir)?;
    entries.sort();

    for target_id in entries.into_iter().filter_map(|path| {
        if !path.join("target.toml").exists() {
            return None;
        }
        path.file_name()
            .and_then(|name| name.to_str())
            .map(str::to_owned)
    }) {
        match TargetId::new(target_id.clone()) {
            Ok(validated_id) => {
                let target_paths = TargetPaths::new(watch_root, validated_id.clone());
                let include_target = match validate_target(&target_paths) {
                    Ok(target) => target.enabled(),
                    Err(_) => true,
                };
                if include_target {
                    targets.push(DiscoveredTarget {
                        requested_id: target_id,
                        validated_id: Some(validated_id),
                        validation_message: None,
                    });
                }
            }
            Err(error) => {
                targets.push(DiscoveredTarget {
                    requested_id: target_id,
                    validated_id: None,
                    validation_message: Some(contract_message(error)),
                });
            }
        }
    }

    Ok(targets)
}

pub(crate) fn collect_watch_root_directories<I>(
    watch_root: &Path,
    entries: I,
) -> Result<Vec<std::path::PathBuf>, CoreError>
where
    I: IntoIterator<Item = std::io::Result<std::path::PathBuf>>,
{
    let mut directories = Vec::new();
    for entry in entries {
        let path = entry.map_err(|error| CoreError::io(watch_root, error))?;
        let metadata = fs::symlink_metadata(&path).map_err(|error| CoreError::io(&path, error))?;
        if metadata.is_dir() {
            directories.push(path);
        }
    }
    Ok(directories)
}

fn run_report_requires_failed_exit(report: &ffhn_core::RunReport) -> bool {
    matches!(
        report.run_outcome(),
        RunOutcome::FailedTransient | RunOutcome::FailedPermanent
    ) || report.run_mode() == RunMode::Live
        && (report.persist().error.is_some() || run_report_has_notification_failures(report))
}

fn batch_report_has_notification_failures(report: &ffhn_core::BatchRunReport) -> bool {
    report
        .entries()
        .iter()
        .filter_map(|entry| entry.run_report())
        .any(run_report_has_notification_failures)
}

fn run_report_has_notification_failures(report: &ffhn_core::RunReport) -> bool {
    report
        .notifications()
        .iter()
        .any(|delivery| !delivery.delivered)
}

fn requested_run_mode(dry_run: bool) -> RunMode {
    if dry_run {
        RunMode::DryRun
    } else {
        RunMode::Live
    }
}

fn render_batch_result(
    batch_result: Result<BatchRunReport, CoreError>,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> i32 {
    match batch_result {
        Ok(report) => {
            if render_json_document(stdout, &report).is_err() {
                let _ = write_cli_error(stderr, &batch_run_report_write_error());
                EXIT_CODE_FATAL
            } else if report.outcome_counts().failed_transient > 0
                || report.outcome_counts().failed_permanent > 0
                || report.outcome_counts().persist_error > 0
                || report.outcome_counts().fatal_error > 0
                || batch_report_has_notification_failures(&report)
            {
                EXIT_CODE_RUN_FAILED
            } else {
                0
            }
        }
        Err(error) => {
            let _ = write_cli_error(stderr, &error.to_string());
            EXIT_CODE_FATAL
        }
    }
}

fn run_discovered_batch(
    watch_root: &Path,
    discovered_targets: Vec<DiscoveredTarget>,
    run_mode: RunMode,
    jobs: usize,
) -> Result<BatchRunReport, CoreError> {
    let valid_targets = discovered_targets
        .iter()
        .filter_map(|target| target.validated_id.clone())
        .collect::<Vec<_>>();
    let requested_targets = discovered_targets
        .iter()
        .map(|target| target.requested_id.clone())
        .collect::<Vec<_>>();
    let base_report = run_batch(watch_root, &valid_targets, run_mode, jobs)?;
    let entries = merge_discovered_entries(discovered_targets, base_report.entries().to_vec());

    BatchRunReport::new(BatchRunReportInput::new(
        run_mode,
        base_report.watch_root().to_owned(),
        requested_targets,
        base_report.run_started_at().to_owned(),
        base_report.run_finished_at().to_owned(),
        base_report.max_concurrency(),
        entries,
    )?)
}

fn merge_discovered_entries(
    discovered_targets: Vec<DiscoveredTarget>,
    valid_entries: Vec<BatchRunEntry>,
) -> Vec<BatchRunEntry> {
    let mut valid_entries = valid_entries.into_iter();

    discovered_targets
        .into_iter()
        .map(|target| match target.validated_id {
            Some(_) => valid_entries
                .next()
                .expect("valid batch entries should align with discovered targets"),
            None => BatchRunEntry::fatal(
                target.requested_id,
                ProcessErrorDetail {
                    kind: ffhn_core::ProcessErrorKind::Contract,
                    message: target.validation_message.unwrap_or_else(|| {
                        "target_id violates FFHN's durable target-id contract".to_owned()
                    }),
                    path: None,
                },
            )
            .expect("discovered fatal entries must accept non-empty target labels"),
        })
        .collect()
}

fn contract_message(error: CoreError) -> String {
    match error {
        CoreError::Contract(message)
        | CoreError::HtmlcutInterop(message)
        | CoreError::Internal(message) => message,
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ffhn_core::{BatchRunReportInput, ProcessErrorKind};

    fn empty_batch_report() -> BatchRunReport {
        BatchRunReport::new(
            BatchRunReportInput::new(
                RunMode::DryRun,
                "watchlist".to_owned(),
                Vec::new(),
                "2026-04-05T10:15:30Z".to_owned(),
                "2026-04-05T10:15:31Z".to_owned(),
                1,
                Vec::new(),
            )
            .expect("batch report input"),
        )
        .expect("empty batch report")
    }

    fn fatal_batch_report() -> BatchRunReport {
        BatchRunReport::new(
            BatchRunReportInput::new(
                RunMode::DryRun,
                "watchlist".to_owned(),
                vec!["bad".to_owned()],
                "2026-04-05T10:15:30Z".to_owned(),
                "2026-04-05T10:15:31Z".to_owned(),
                1,
                vec![
                    BatchRunEntry::fatal(
                        "bad",
                        ProcessErrorDetail {
                            kind: ProcessErrorKind::Contract,
                            message: "bad".to_owned(),
                            path: None,
                        },
                    )
                    .expect("fatal entry"),
                ],
            )
            .expect("batch report input"),
        )
        .expect("fatal batch report")
    }

    #[test]
    fn batch_rendering_helpers_cover_success_failure_and_error_paths() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        assert_eq!(
            render_batch_result(Ok(empty_batch_report()), &mut stdout, &mut stderr),
            0
        );
        assert!(stderr.is_empty());

        stdout.clear();
        stderr.clear();
        assert_eq!(
            render_batch_result(Ok(fatal_batch_report()), &mut stdout, &mut stderr),
            EXIT_CODE_RUN_FAILED
        );
        assert!(stderr.is_empty());

        stdout.clear();
        stderr.clear();
        assert_eq!(
            render_batch_result(Err(CoreError::internal("boom")), &mut stdout, &mut stderr,),
            EXIT_CODE_FATAL
        );
        assert!(stdout.is_empty());
        assert!(
            String::from_utf8(stderr)
                .expect("stderr utf8")
                .contains("boom")
        );
    }

    #[test]
    fn helper_functions_cover_mode_selection_merge_and_error_messages() {
        assert_eq!(requested_run_mode(true), RunMode::DryRun);
        assert_eq!(requested_run_mode(false), RunMode::Live);

        let merged = merge_discovered_entries(
            vec![DiscoveredTarget {
                requested_id: "Demo".to_owned(),
                validated_id: None,
                validation_message: Some("bad target id".to_owned()),
            }],
            Vec::new(),
        );
        assert_eq!(merged[0].target_id(), "Demo");
        let fatal_error = merged[0].fatal_error().expect("fatal error");
        assert_eq!(fatal_error.kind, ProcessErrorKind::Contract);
        assert_eq!(fatal_error.message, "bad target id");

        let merged = merge_discovered_entries(
            vec![DiscoveredTarget {
                requested_id: "escape".to_owned(),
                validated_id: None,
                validation_message: None,
            }],
            Vec::new(),
        );
        assert_eq!(
            merged[0].fatal_error().expect("fatal error").message,
            "target_id violates FFHN's durable target-id contract"
        );

        assert_eq!(
            contract_message(CoreError::contract("bad target")),
            "bad target"
        );
        assert_eq!(
            contract_message(CoreError::htmlcut_interop("bad htmlcut")),
            "bad htmlcut"
        );
        assert_eq!(
            contract_message(CoreError::internal("bad state")),
            "bad state"
        );
        assert!(
            contract_message(CoreError::io("watchlist", io::Error::other("boom")))
                .contains("filesystem error")
        );
    }
}
