use super::notification::{NotificationDeliveryOutcome, RunNotificationDelivery};
use super::*;

pub(super) fn validate_run_report_identity(name: &str, version: u32) -> Result<(), CoreError> {
    validate_identity(
        name,
        RUN_REPORT_SCHEMA_NAME,
        version,
        RUN_REPORT_SCHEMA_VERSION,
    )
}

pub(super) fn validate_status_report_identity(name: &str, version: u32) -> Result<(), CoreError> {
    validate_identity(
        name,
        STATUS_REPORT_SCHEMA_NAME,
        version,
        STATUS_REPORT_SCHEMA_VERSION,
    )
}

pub(super) fn validate_batch_run_report_identity(
    name: &str,
    version: u32,
) -> Result<(), CoreError> {
    validate_identity(
        name,
        BATCH_RUN_REPORT_SCHEMA_NAME,
        version,
        BATCH_RUN_REPORT_SCHEMA_VERSION,
    )
}

pub(crate) fn validate_batch_request_contract(
    requested_targets: &[String],
    max_concurrency: usize,
) -> Result<(), CoreError> {
    if max_concurrency == 0 {
        return Err(CoreError::contract(
            "batch.max_concurrency must be positive",
        ));
    }

    let mut seen = BTreeSet::new();
    for target_id in requested_targets {
        require_non_empty("batch.requested_targets entry", target_id)?;
        if !seen.insert(target_id.as_str()) {
            return Err(CoreError::contract(
                "batch.requested_targets values must be unique",
            ));
        }
    }

    Ok(())
}

pub(super) fn validate_run_change_section(change: &RunChangeSection) -> Result<(), CoreError> {
    let previous_line_count = change.previous_line_count.unwrap_or(0);

    match change.kind {
        ChangeKind::Initialized => {
            if change.previous_text_bytes.is_some() || change.previous_line_count.is_some() {
                return Err(CoreError::contract(
                    "run_report.change initialized must not carry previous text counts",
                ));
            }
        }
        ChangeKind::Changed | ChangeKind::Unchanged => {
            if change.previous_text_bytes.is_none() || change.previous_line_count.is_none() {
                return Err(CoreError::contract(
                    "run_report.change changed and unchanged must carry previous text counts",
                ));
            }
        }
    }

    if change.current_line_count == 0 && change.current_text_bytes > 0 {
        return Err(CoreError::contract(
            "run_report.change current_line_count must be positive when text exists",
        ));
    }
    if let Some(previous_line_count) = change.previous_line_count
        && change.previous_text_bytes.unwrap_or(0) > 0
        && previous_line_count == 0
    {
        return Err(CoreError::contract(
            "run_report.change previous_line_count must be positive when previous text exists",
        ));
    }

    if change.common_prefix_lines > previous_line_count
        || change.common_prefix_lines > change.current_line_count
    {
        return Err(CoreError::contract(
            "run_report.change common_prefix_lines must fit within both sides",
        ));
    }

    let previous_remaining = previous_line_count.saturating_sub(change.common_prefix_lines);
    let current_remaining = change
        .current_line_count
        .saturating_sub(change.common_prefix_lines);
    if change.common_suffix_lines > previous_remaining
        || change.common_suffix_lines > current_remaining
    {
        return Err(CoreError::contract(
            "run_report.change common_suffix_lines must fit within the remaining lines on both sides",
        ));
    }

    if let Some(region) = &change.changed_region {
        let expected_previous_region =
            previous_line_count - change.common_prefix_lines - change.common_suffix_lines;
        let expected_current_region =
            change.current_line_count - change.common_prefix_lines - change.common_suffix_lines;
        if region.previous_start_line != change.common_prefix_lines + 1
            || region.current_start_line != change.common_prefix_lines + 1
        {
            return Err(CoreError::contract(
                "run_report.change changed_region must start immediately after the common prefix",
            ));
        }
        if region.previous_line_count != expected_previous_region
            || region.current_line_count != expected_current_region
        {
            return Err(CoreError::contract(
                "run_report.change changed_region counts must match the non-common line region",
            ));
        }
        if matches!(
            (
                region.current_line_count > 0,
                region.current_excerpt.as_ref(),
                region.current_excerpt_sha256.as_ref(),
            ),
            (true, Some(_), None)
        ) {
            return Err(CoreError::contract(
                "run_report.change changed_region current excerpts require a digest",
            ));
        }
        if matches!(
            (
                region.previous_line_count > 0,
                region.previous_excerpt.as_ref(),
                region.previous_excerpt_sha256.as_ref(),
            ),
            (true, Some(_), None)
        ) {
            return Err(CoreError::contract(
                "run_report.change changed_region previous excerpts require a digest",
            ));
        }
        region
            .current_excerpt_sha256
            .as_deref()
            .map(validate_sha256)
            .transpose()?;
        region
            .previous_excerpt_sha256
            .as_deref()
            .map(validate_sha256)
            .transpose()?;
    }
    Ok(())
}

pub(super) fn validate_notification_delivery(
    delivery: &RunNotificationDelivery,
) -> Result<(), CoreError> {
    require_non_empty("notifications.route_name", &delivery.route_name)?;
    match &delivery.outcome {
        NotificationDeliveryOutcome::Delivered { exit_code } => {
            if *exit_code != 0 {
                return Err(CoreError::contract(
                    "delivered notifications must exit with code 0",
                ));
            }
        }
        NotificationDeliveryOutcome::TimedOut { error } => {
            require_non_empty("notifications.error", error)?;
        }
        NotificationDeliveryOutcome::Failed { exit_code, error } => {
            require_non_empty("notifications.error", error)?;
            if matches!(exit_code, Some(0)) {
                return Err(CoreError::contract(
                    "failed notifications must not report exit_code = 0",
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_change() -> RunChangeSection {
        RunChangeSection {
            kind: ChangeKind::Changed,
            previous_text_bytes: Some(6),
            current_text_bytes: 7,
            previous_line_count: Some(3),
            current_line_count: 3,
            common_prefix_lines: 1,
            common_suffix_lines: 1,
            changed_region: Some(RunChangeRegion {
                previous_start_line: 2,
                previous_line_count: 1,
                current_start_line: 2,
                current_line_count: 1,
                previous_excerpt: Some("before".to_owned()),
                current_excerpt: Some("after".to_owned()),
                previous_excerpt_sha256: Some(
                    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
                ),
                current_excerpt_sha256: Some(
                    "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_owned(),
                ),
            }),
        }
    }

    #[test]
    fn change_and_notification_validators_reject_the_full_invalid_surface() {
        validate_run_change_section(&valid_change()).expect("valid changed change");

        let valid_initialized = RunChangeSection {
            kind: ChangeKind::Initialized,
            previous_text_bytes: None,
            previous_line_count: None,
            current_text_bytes: 7,
            current_line_count: 3,
            common_prefix_lines: 0,
            common_suffix_lines: 0,
            changed_region: None,
        };
        validate_run_change_section(&valid_initialized).expect("valid initialized change");

        let valid_unchanged = RunChangeSection {
            kind: ChangeKind::Unchanged,
            ..valid_change()
        };
        validate_run_change_section(&valid_unchanged).expect("valid unchanged change");

        let initialized_with_previous = RunChangeSection {
            kind: ChangeKind::Initialized,
            previous_text_bytes: Some(1),
            previous_line_count: Some(1),
            ..valid_change()
        };
        assert!(validate_run_change_section(&initialized_with_previous).is_err());

        let initialized_with_only_previous_lines = RunChangeSection {
            kind: ChangeKind::Initialized,
            previous_text_bytes: None,
            previous_line_count: Some(1),
            ..valid_change()
        };
        assert!(validate_run_change_section(&initialized_with_only_previous_lines).is_err());

        let changed_without_previous = RunChangeSection {
            previous_text_bytes: None,
            previous_line_count: None,
            ..valid_change()
        };
        assert!(validate_run_change_section(&changed_without_previous).is_err());

        let changed_without_previous_line_count = RunChangeSection {
            previous_text_bytes: Some(6),
            previous_line_count: None,
            ..valid_change()
        };
        assert!(validate_run_change_section(&changed_without_previous_line_count).is_err());

        let zero_current_lines = RunChangeSection {
            current_text_bytes: 1,
            current_line_count: 0,
            changed_region: None,
            ..valid_change()
        };
        assert!(validate_run_change_section(&zero_current_lines).is_err());

        let zero_previous_lines = RunChangeSection {
            previous_text_bytes: Some(1),
            previous_line_count: Some(0),
            changed_region: None,
            ..valid_change()
        };
        assert!(validate_run_change_section(&zero_previous_lines).is_err());

        let too_large_prefix = RunChangeSection {
            common_prefix_lines: 4,
            changed_region: None,
            ..valid_change()
        };
        assert!(validate_run_change_section(&too_large_prefix).is_err());

        let prefix_exceeds_current_only = RunChangeSection {
            previous_line_count: Some(5),
            current_line_count: 3,
            common_prefix_lines: 4,
            changed_region: None,
            ..valid_change()
        };
        assert!(validate_run_change_section(&prefix_exceeds_current_only).is_err());

        let too_large_suffix = RunChangeSection {
            common_prefix_lines: 1,
            common_suffix_lines: 3,
            changed_region: None,
            ..valid_change()
        };
        assert!(validate_run_change_section(&too_large_suffix).is_err());

        let suffix_exceeds_current_only = RunChangeSection {
            previous_line_count: Some(5),
            current_line_count: 3,
            common_prefix_lines: 1,
            common_suffix_lines: 3,
            changed_region: None,
            ..valid_change()
        };
        assert!(validate_run_change_section(&suffix_exceeds_current_only).is_err());

        let mut wrong_start = valid_change();
        let region = wrong_start.changed_region.as_mut().expect("region");
        region.previous_start_line = 1;
        assert!(validate_run_change_section(&wrong_start).is_err());

        let mut wrong_current_start = valid_change();
        let region = wrong_current_start.changed_region.as_mut().expect("region");
        region.current_start_line = 1;
        assert!(validate_run_change_section(&wrong_current_start).is_err());

        let mut wrong_counts = valid_change();
        let region = wrong_counts.changed_region.as_mut().expect("region");
        region.current_line_count = 2;
        assert!(validate_run_change_section(&wrong_counts).is_err());

        let mut wrong_previous_count = valid_change();
        let region = wrong_previous_count
            .changed_region
            .as_mut()
            .expect("region");
        region.previous_line_count = 2;
        assert!(validate_run_change_section(&wrong_previous_count).is_err());

        let mut missing_current_digest = valid_change();
        let region = missing_current_digest
            .changed_region
            .as_mut()
            .expect("region");
        region.current_excerpt_sha256 = None;
        assert!(validate_run_change_section(&missing_current_digest).is_err());

        let mut missing_previous_digest = valid_change();
        let region = missing_previous_digest
            .changed_region
            .as_mut()
            .expect("region");
        region.previous_excerpt_sha256 = None;
        assert!(validate_run_change_section(&missing_previous_digest).is_err());

        assert!(
            validate_notification_delivery(&RunNotificationDelivery {
                route_name: "notify".to_owned(),
                duration_ms: 1,
                outcome: NotificationDeliveryOutcome::Delivered { exit_code: 7 },
            })
            .is_err()
        );
        validate_notification_delivery(&RunNotificationDelivery {
            route_name: "notify".to_owned(),
            duration_ms: 1,
            outcome: NotificationDeliveryOutcome::Delivered { exit_code: 0 },
        })
        .expect("delivered notification");

        assert!(
            validate_notification_delivery(&RunNotificationDelivery {
                route_name: "notify".to_owned(),
                duration_ms: 1,
                outcome: NotificationDeliveryOutcome::TimedOut {
                    error: String::new(),
                },
            })
            .is_err()
        );
        validate_notification_delivery(&RunNotificationDelivery {
            route_name: "notify".to_owned(),
            duration_ms: 1,
            outcome: NotificationDeliveryOutcome::TimedOut {
                error: "timed out".to_owned(),
            },
        })
        .expect("timed out notification");

        assert!(
            validate_notification_delivery(&RunNotificationDelivery {
                route_name: "notify".to_owned(),
                duration_ms: 1,
                outcome: NotificationDeliveryOutcome::Failed {
                    exit_code: Some(0),
                    error: "failed".to_owned(),
                },
            })
            .is_err()
        );
        validate_notification_delivery(&RunNotificationDelivery {
            route_name: "notify".to_owned(),
            duration_ms: 1,
            outcome: NotificationDeliveryOutcome::Failed {
                exit_code: Some(7),
                error: "failed".to_owned(),
            },
        })
        .expect("failed notification");
    }
}
