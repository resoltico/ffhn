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
        return Err(CoreError::htmlcut("batch.max_concurrency must be positive"));
    }

    let mut seen = BTreeSet::new();
    for target_id in requested_targets {
        validate_target_id(target_id)?;
        if !seen.insert(target_id.as_str()) {
            return Err(CoreError::htmlcut(
                "batch.requested_targets values must be unique",
            ));
        }
    }

    Ok(())
}

pub(super) fn validate_run_change_section(change: &RunChangeSection) -> Result<(), CoreError> {
    if change.current_line_count == 0 && change.current_text_bytes > 0 {
        return Err(CoreError::htmlcut(
            "run_report.change current_line_count must be positive when text exists",
        ));
    }
    if let Some(previous_line_count) = change.previous_line_count
        && change.previous_text_bytes.unwrap_or(0) > 0
        && previous_line_count == 0
    {
        return Err(CoreError::htmlcut(
            "run_report.change previous_line_count must be positive when previous text exists",
        ));
    }
    if let Some(region) = &change.changed_region {
        if matches!(
            (
                region.current_line_count > 0,
                region.current_excerpt.as_ref(),
                region.current_excerpt_sha256.as_ref(),
            ),
            (true, Some(_), None)
        ) {
            return Err(CoreError::htmlcut(
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
            return Err(CoreError::htmlcut(
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
    require_non_empty("notifications.hook_name", &delivery.hook_name)?;
    match (delivery.delivered, delivery.timed_out, delivery.exit_code) {
        (true, true, _) => {
            return Err(CoreError::htmlcut(
                "notifications cannot be both delivered and timed_out",
            ));
        }
        (true, false, Some(0)) | (false, _, _) => {}
        (true, false, _) => {
            return Err(CoreError::htmlcut(
                "delivered notifications must exit with code 0",
            ));
        }
    }
    Ok(())
}
