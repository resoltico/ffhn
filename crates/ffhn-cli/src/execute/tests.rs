use std::io;

use ffhn_core::{
    BatchRunEntry, BatchRunReport, BatchRunReportInput, CoreError, ProcessErrorKind, RunMode,
};

use super::batch::{merge_discovered_entries, render_batch_result, requested_run_mode};
use super::discovery::{DiscoveredTarget, contract_message};
use super::*;

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
                    ffhn_core::ProcessErrorDetail::new(ProcessErrorKind::Contract, "bad", None)
                        .expect("fatal detail"),
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
    assert_eq!(fatal_error.kind(), ProcessErrorKind::Contract);
    assert_eq!(fatal_error.message(), "bad target id");

    let merged = merge_discovered_entries(
        vec![DiscoveredTarget {
            requested_id: "escape".to_owned(),
            validated_id: None,
            validation_message: None,
        }],
        Vec::new(),
    );
    assert_eq!(
        merged[0].fatal_error().expect("fatal error").message(),
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
